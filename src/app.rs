use std::time::{Duration, Instant};

use serde::Serialize;
use topcoat::{
    Result,
    asset::{Asset, CxAssetExt, asset},
    context::Cx,
    htmx::hx_boosted,
    react::{ReactComponent, ReactServerRenderer},
    router::{HeaderValue, content::Json, header, query_params, request::headers, route},
    tailwind,
    view::{View, component, defer_script, view},
};

const TYPEAHEAD_CSS: Asset = asset!("assets/typeahead.css");
const TYPEAHEAD_JS: Asset = asset!("assets/typeahead.js");
const TYPEAHEAD: ReactComponent<TypeaheadProps> =
    ReactComponent::new("catalog-typeahead", TYPEAHEAD_JS).server_renderer(
        ReactServerRenderer::new(include_str!("../assets/typeahead.ssr.js")),
    );
const SUGGESTIONS_PATH: &str = "/api/suggestions";

#[query_params(error = redirect("?mode=streaming"))]
struct CatalogQuery {
    mode: Option<String>,
    category: Option<String>,
    product: Option<String>,
    q: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RenderMode {
    Sequential,
    Concurrent,
    Streaming,
}

impl RenderMode {
    fn from_query(value: Option<&str>) -> Self {
        match value {
            Some("sequential") => Self::Sequential,
            Some("concurrent") => Self::Concurrent,
            _ => Self::Streaming,
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Self::Sequential => "sequential",
            Self::Concurrent => "concurrent",
            Self::Streaming => "streaming",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Sequential => "Sequential",
            Self::Concurrent => "Concurrent SSR",
            Self::Streaming => "Deferred View",
        }
    }

    fn expected(self) -> &'static str {
        match self {
            Self::Sequential => "about 1.8 s",
            Self::Concurrent => "about 550 ms",
            Self::Streaming => "shell immediately",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Category {
    slug: &'static str,
    name: &'static str,
    description: &'static str,
    count: usize,
}

#[derive(Clone, Copy)]
struct Product {
    category: &'static str,
    sku: &'static str,
    name: &'static str,
    specification: &'static str,
    price: &'static str,
    unit: &'static str,
    stock: usize,
    delay_ms: u64,
}

#[derive(Serialize)]
struct ProductSuggestion {
    name: &'static str,
    sku: &'static str,
    specification: &'static str,
    href: String,
}

#[derive(Serialize)]
struct TypeaheadProps {
    initial_value: String,
    suggestions_url: String,
}

#[route(GET "/api/suggestions")]
async fn suggestions(cx: &Cx) -> Result<Json<Vec<ProductSuggestion>>> {
    let query = query_params::<CatalogQuery>(cx)?;
    let mode = RenderMode::from_query(query.mode.as_deref());
    Ok(Json(product_suggestions(mode)))
}

#[route(GET "/")]
async fn catalog(cx: &Cx) -> Result {
    let query = query_params::<CatalogQuery>(cx)?;
    let mode = RenderMode::from_query(query.mode.as_deref());
    let category = find_category(query.category.as_deref());
    let request_kind = request_kind(cx);

    if let Some(product) = query.product.as_deref().and_then(find_product) {
        return product_page(cx, mode, category, product, request_kind).await;
    }

    let search = query.q.clone();
    match mode {
        RenderMode::Sequential => {
            sequential_catalog(cx, mode, category, search, request_kind).await
        }
        RenderMode::Concurrent => {
            Box::pin(concurrent_catalog(cx, mode, category, search, request_kind)).await
        }
        RenderMode::Streaming => streaming_catalog(cx, mode, category, search, request_kind).await,
    }
}

async fn sequential_catalog(
    cx: &Cx,
    mode: RenderMode,
    category: Category,
    search: Option<String>,
    request_kind: &'static str,
) -> Result {
    let started = Instant::now();
    let inventory_view = view! { cx => inventory_summary(category: category) }?;
    let products_view = view! {
        cx =>
        product_table(
            category: category,
            search: search.clone(),
            concurrent: false,
            mode: mode
        )
    }?;
    let sourcing_view = view! { cx => sourcing_notes() }?;
    let server_ms = started.elapsed().as_millis();
    let content = catalog_layout(
        cx,
        mode,
        category,
        search.as_deref(),
        inventory_view,
        products_view,
        sourcing_view,
    )
    .await?;
    let document = document(
        cx,
        mode,
        category,
        search.as_deref(),
        request_kind,
        Some(server_ms),
        content,
    )
    .await?;
    Ok(document)
}

async fn concurrent_catalog(
    cx: &Cx,
    mode: RenderMode,
    category: Category,
    search: Option<String>,
    request_kind: &'static str,
) -> Result {
    let started = Instant::now();
    let content = view! {
        cx =>
        <div class="grid gap-5 xl:grid-cols-[minmax(0,1fr)_18rem]">
            <div class="space-y-5">
                inventory_summary(category: category)
                product_table(
                    category: category,
                    search: search.clone(),
                    concurrent: true,
                    mode: mode
                )
            </div>
            sourcing_notes()
        </div>
    }?;
    let server_ms = started.elapsed().as_millis();
    let content = catalog_chrome(cx, mode, category, search.as_deref(), content).await?;
    let document = document(
        cx,
        mode,
        category,
        search.as_deref(),
        request_kind,
        Some(server_ms),
        content,
    )
    .await?;
    Ok(document)
}

async fn streaming_catalog(
    cx: &Cx,
    mode: RenderMode,
    category: Category,
    search: Option<String>,
    request_kind: &'static str,
) -> Result {
    let products_placeholder = product_table_placeholder(cx).await?;
    let inventory_slot = inventory_placeholder().await?.defer(move |cx| async move {
        let cx = cx.as_ref();
        view! { cx => inventory_summary(category: category) }
    });
    let sourcing_slot = sourcing_placeholder().await?.defer(|cx| async move {
        let cx = cx.as_ref();
        view! { cx => sourcing_notes() }
    });

    let product_search = search.clone();
    let content = view! {
        cx =>
        <div class="grid gap-5 xl:grid-cols-[minmax(0,1fr)_18rem]">
            <div class="space-y-5">
                (inventory_slot)
                defer product_table(category: category, search: product_search, concurrent: true, mode: mode) {
                    (products_placeholder)
                }
            </div>
            (sourcing_slot)
        </div>
    }?;
    let content = catalog_chrome(cx, mode, category, search.as_deref(), content).await?;
    document(
        cx,
        mode,
        category,
        search.as_deref(),
        request_kind,
        None,
        content,
    )
    .await
}

async fn product_page(
    cx: &Cx,
    mode: RenderMode,
    category: Category,
    product: Product,
    request_kind: &'static str,
) -> Result {
    let started = Instant::now();
    let content = view! {
        cx =>
        product_detail(product: product, mode: mode, category: category)
    }?;
    let server_ms = started.elapsed().as_millis();
    let document = document(
        cx,
        mode,
        category,
        None,
        request_kind,
        Some(server_ms),
        content,
    )
    .await?;
    Ok(document)
}

async fn document(
    cx: &Cx,
    mode: RenderMode,
    category: Category,
    search: Option<&str>,
    request_kind: &'static str,
    server_ms: Option<u128>,
    content: View,
) -> Result {
    cx.require_asset(TYPEAHEAD_CSS.stylesheet())?;
    let typeahead = deferred_typeahead(cx, mode, search.unwrap_or("")).await?;
    let sequential_item = mode_item_class(mode == RenderMode::Sequential);
    let concurrent_item = mode_item_class(mode == RenderMode::Concurrent);
    let streaming_item = mode_item_class(mode == RenderMode::Streaming);

    view! {
        cx =>
        ((
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, max-age=30"),
        ))
        <!DOCTYPE html>
        <html lang="en" class="bg-stone-100">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <meta name="color-scheme" content="light">
                <title>"Kitchen Sink Supply"</title>
                <link rel="stylesheet" href=(tailwind::stylesheet!())>
                <script src=(asset!("assets/htmx.min.js"))></script>
                <script src=(asset!("assets/dashboard.js"))></script>
                defer_script()
                topcoat::dev::script()
            </head>
            <body
                hx-boost="true"
                hx-ext="preload"
                hx-indicator="#page-progress"
                class="min-h-screen bg-stone-100 font-sans text-slate-950 antialiased"
            >
                <div
                    id="page-progress"
                    class="htmx-indicator fixed inset-x-0 top-0 z-50 h-1 origin-left bg-amber-400 shadow-lg shadow-amber-400/50"
                ></div>
                <header class="border-b border-slate-950/20 bg-amber-300 shadow-sm">
                    <div class="bg-slate-950 text-white">
                        <div
                            class="mx-auto flex h-11 max-w-7xl items-center justify-between px-4 sm:px-6 lg:px-8"
                        >
                            <a
                                href=(catalog_href(mode, category))
                                preload="mouseover"
                                class="flex items-center gap-2 font-bold tracking-tight"
                            >
                                <img
                                    src=(asset!("assets/mark.svg"))
                                    alt=""
                                    class="size-7 rounded-lg"
                                >
                                <span>"Kitchen Sink Supply"</span>
                                <span
                                    class="hidden text-xs font-medium text-slate-400 sm:inline"
                                >
                                    "kitchen sink"
                                </span>
                            </a>
                            <div class="flex items-center gap-2">
                                <div class="hidden items-center gap-2 text-xs md:flex">
                                    <span
                                        class="rounded-full bg-white/10 px-2.5 py-1 text-slate-300"
                                    >
                                        (mode.label())
                                    </span>
                                    <span class="text-slate-500">"expected"</span>
                                    <span class="font-semibold text-amber-300">
                                        (mode.expected())
                                    </span>
                                    <span class="text-slate-600">"/"</span>
                                    <span class="text-cyan-300">(request_kind)</span>
                                </div>
                                <details class="group relative">
                                    <summary
                                        class="flex cursor-pointer list-none items-center gap-2 rounded-md border border-white/15 bg-white/5 px-3 py-1.5 text-sm font-semibold text-slate-200 hover:bg-white/10"
                                    >
                                        <span class="grid w-4 gap-1" aria-hidden="true">
                                            <span class="h-px w-4 bg-current"></span>
                                            <span class="h-px w-4 bg-current"></span>
                                            <span class="h-px w-4 bg-current"></span>
                                        </span>
                                        <span class="hidden sm:inline">"Render mode"</span>
                                    </summary>
                                    <div
                                        class="absolute right-0 z-40 mt-2 w-72 rounded-xl border border-slate-200 bg-white p-2 text-slate-950 shadow-2xl shadow-slate-950/30"
                                    >
                                        <p
                                            class="px-3 pb-2 pt-1 text-xs font-bold uppercase tracking-wider text-slate-400"
                                        >
                                            "Load this catalog"
                                        </p>
                                        <a
                                            href=(mode_href(RenderMode::Sequential, category))
                                            hx-boost="false"
                                            class=(sequential_item)
                                            if mode == RenderMode::Sequential {
                                                aria-current="page"
                                            }
                                        >
                                            <span class="font-semibold">"Sequential"</span>
                                            <span class="mt-0.5 block text-xs opacity-60">
                                                "Await every source in order"
                                            </span>
                                        </a>
                                        <a
                                            href=(mode_href(RenderMode::Concurrent, category))
                                            hx-boost="false"
                                            class=(concurrent_item)
                                            if mode == RenderMode::Concurrent {
                                                aria-current="page"
                                            }
                                        >
                                            <span class="font-semibold">"Concurrent SSR"</span>
                                            <span class="mt-0.5 block text-xs opacity-60">
                                                "Join peers and product rows"
                                            </span>
                                        </a>
                                        <a
                                            href=(mode_href(RenderMode::Streaming, category))
                                            hx-boost="false"
                                            class=(streaming_item)
                                            if mode == RenderMode::Streaming {
                                                aria-current="page"
                                            }
                                        >
                                            <span class="font-semibold">"Deferred View"</span>
                                            <span class="mt-0.5 block text-xs opacity-60">
                                                "Stream into nested slots"
                                            </span>
                                        </a>
                                    </div>
                                </details>
                            </div>
                        </div>
                    </div>
                    <div
                        class="mx-auto flex max-w-7xl items-center gap-5 px-4 py-4 sm:px-6 lg:px-8"
                    >
                        <div class="hidden shrink-0 lg:block">
                            <p
                                class="text-xs font-bold uppercase tracking-widest text-amber-950/60"
                            >
                                "Industrial catalog"
                            </p>
                            <p class="text-xl font-black tracking-tight text-slate-950">
                                "1,248,390 products"
                            </p>
                        </div>
                        <form
                            action="/"
                            method="get"
                            class="flex min-w-0 flex-1 overflow-hidden rounded-md border-2 border-slate-950 bg-white shadow-sm"
                        >
                            <input type="hidden" name="mode" value=(mode.slug())>
                            <input type="hidden" name="category" value=(category.slug)>
                            <label for="catalog-search" class="sr-only">
                                "Search products"
                            </label>
                            (typeahead)
                            <button
                                class="bg-slate-950 px-5 text-sm font-bold text-white hover:bg-slate-800"
                            >
                                "Search"
                            </button>
                        </form>
                        <div
                            class="hidden items-center gap-5 text-sm font-semibold text-amber-950 lg:flex"
                        >
                            <a href="#" class="hover:underline">"Help"</a>
                            <a href="#" class="hover:underline">"Orders"</a>
                            <span
                                class="rounded-md border border-amber-950/20 bg-amber-200 px-3 py-2"
                            >
                                "Cart 0"
                            </span>
                        </div>
                    </div>
                </header>

                <main
                    id="catalog-main"
                    class="mx-auto max-w-7xl px-4 py-5 sm:px-6 lg:px-8"
                >
                    <div
                        class="mb-4 flex flex-wrap items-center justify-between gap-3 text-xs"
                    >
                        <div class="flex items-center gap-2 text-slate-500">
                            <a
                                href=(catalog_href(mode, all_category()))
                                preload="mouseover"
                                class="font-semibold text-slate-700 hover:underline"
                            >
                                "Catalog"
                            </a>
                            <span>"/"</span>
                            <span>(category.name)</span>
                        </div>
                        <div class="flex items-center gap-2">
                            <span
                                class="rounded-full border border-slate-200 bg-white px-2.5 py-1 text-slate-500 md:hidden"
                            >
                                (mode.label())
                            </span>
                            if let Some(server_ms) = server_ms {
                                <span
                                    class="rounded-full border border-slate-200 bg-white px-2.5 py-1 text-slate-500"
                                >
                                    "server "
                                    (server_ms)
                                    " ms"
                                </span>
                            } else {
                                <span
                                    class="rounded-full bg-cyan-50 px-2.5 py-1 font-semibold text-cyan-700"
                                >
                                    "streaming response"
                                </span>
                            }
                            <span
                                class="rounded-full bg-emerald-50 px-2.5 py-1 font-semibold text-emerald-700"
                            >
                                "hover links to preload"
                            </span>
                        </div>
                    </div>
                    (content)
                    <footer
                        class="mt-10 flex flex-col gap-2 border-t border-slate-300 py-6 text-xs text-slate-500 sm:flex-row sm:justify-between"
                    >
                        <p>
                            "Demo catalog. Same data sources, three scheduling strategies."
                        </p>
                        <p>
                            "Topcoat + deferred views + htmx boost + hover preload + Tailwind CSS"
                        </p>
                    </footer>
                </main>
            </body>
        </html>
    }
}

async fn deferred_typeahead(cx: &Cx, mode: RenderMode, initial_value: &str) -> Result<View> {
    cx.require_asset(TYPEAHEAD_JS.module())?;
    let suggestions_url = format!("{SUGGESTIONS_PATH}?mode={}", mode.slug());
    let initial_value = initial_value.to_owned();
    let placeholder_value = initial_value.clone();
    let island = TYPEAHEAD
        .props(TypeaheadProps {
            initial_value,
            suggestions_url: suggestions_url.clone(),
        })
        .class("contents")
        .preload(cx, suggestions_url, &product_suggestions(mode))?;
    let placeholder = view! {
        cx =>
        <input
            id="catalog-search"
            name="q"
            value=(placeholder_value)
            autocomplete="off"
            role="combobox"
            aria-autocomplete="list"
            aria-expanded="false"
            placeholder="Search by product, material, or specification"
            class="min-w-0 flex-1 px-4 py-2.5 text-sm outline-none placeholder:text-slate-400"
        >
    }?;

    Ok(placeholder.defer(move |cx| async move { island.render(&cx).await }))
}

async fn catalog_layout(
    cx: &Cx,
    mode: RenderMode,
    category: Category,
    search: Option<&str>,
    inventory: View,
    products: View,
    sourcing: View,
) -> Result {
    let results = view! {
        cx =>
        <div class="grid gap-5 xl:grid-cols-[minmax(0,1fr)_18rem]">
            <div class="space-y-5">
                (inventory)
                (products)
            </div>
            (sourcing)
        </div>
    }?;
    catalog_chrome(cx, mode, category, search, results).await
}

async fn catalog_chrome(
    cx: &Cx,
    mode: RenderMode,
    category: Category,
    search: Option<&str>,
    results: View,
) -> Result {
    view! {
        cx =>
        <div class="grid gap-5 lg:grid-cols-[15rem_minmax(0,1fr)]">
            <aside
                class="self-start rounded-lg border border-slate-300 bg-white shadow-sm"
            >
                <div class="border-b border-slate-200 bg-slate-50 px-4 py-3">
                    <p
                        class="text-xs font-bold uppercase tracking-wider text-slate-500"
                    >
                        "Browse categories"
                    </p>
                </div>
                <nav class="p-2" aria-label="Product categories">
                    for item in categories() {
                        <a
                            href=(catalog_href(mode, item))
                            preload="mouseover"
                            class=(category_link_class(item == category))
                            if item == category {
                                aria-current="page"
                            }
                        >
                            <span class="font-semibold">(item.name)</span>
                            <span class="text-xs text-slate-400">(item.count)</span>
                        </a>
                    }
                </nav>
                <div
                    class="border-t border-slate-200 p-4 text-xs leading-5 text-slate-500"
                >
                    <p class="font-semibold text-slate-700">"Fast navigation lab"</p>
                    <p class="mt-1">
                        "Category and product links use hx-boost and preload after a 100 ms hover."
                    </p>
                </div>
            </aside>
            <section>
                <div class="mb-4">
                    <p
                        class="text-xs font-bold uppercase tracking-wider text-amber-700"
                    >
                        "Catalog section"
                    </p>
                    <h1 class="mt-1 text-3xl font-black tracking-tight text-slate-950">
                        (category.name)
                    </h1>
                    <p class="mt-1 max-w-2xl text-sm leading-6 text-slate-600">
                        (category.description)
                    </p>
                    if let Some(search) = search {
                        <p class="mt-2 text-sm font-semibold text-slate-700">
                            "Filtered by: "
                            (search)
                        </p>
                    }
                </div>
                (results)
            </section>
        </div>
    }
}

#[component]
async fn inventory_summary(category: Category) -> Result {
    tokio::time::sleep(Duration::from_millis(350)).await;

    view! {
        <section
            data-arrival="inventory"
            class="grid gap-px overflow-hidden rounded-lg border border-slate-300 bg-slate-300 shadow-sm sm:grid-cols-3"
        >
            <div class="bg-white px-5 py-4">
                <p class="text-xs font-bold uppercase tracking-wider text-slate-400">
                    "Products indexed"
                </p>
                <p class="mt-1 text-2xl font-black tracking-tight">(category.count)</p>
            </div>
            <div class="bg-white px-5 py-4">
                <p class="text-xs font-bold uppercase tracking-wider text-slate-400">
                    "Ready to ship"
                </p>
                <p class="mt-1 text-2xl font-black tracking-tight text-emerald-700">
                    "98.6%"
                </p>
            </div>
            <div class="bg-white px-5 py-4">
                <p class="text-xs font-bold uppercase tracking-wider text-slate-400">
                    "Source latency"
                </p>
                <p class="mt-1 text-2xl font-black tracking-tight">"350 ms"</p>
            </div>
        </section>
    }
}

#[component]
async fn product_table(
    category: Category,
    search: Option<String>,
    concurrent: bool,
    mode: RenderMode,
) -> Result {
    let products = matching_products(category, search.as_deref());
    let mut sequential_rows = Vec::new();
    if !concurrent {
        for product in &products {
            sequential_rows.push(view! {
                product_row(product: *product, mode: mode, category: category)
            }?);
        }
    }

    view! {
        <section
            data-arrival="products"
            class="overflow-hidden rounded-lg border border-slate-300 bg-white shadow-sm"
        >
            <div
                class="flex items-center justify-between border-b border-slate-300 bg-slate-50 px-4 py-3"
            >
                <div>
                    <h2 class="font-bold">"Products and specifications"</h2>
                    <p class="text-xs text-slate-500">
                        (products.len())
                        " matching stocked items"
                    </p>
                </div>
                <span
                    class="rounded bg-amber-100 px-2 py-1 text-xs font-bold text-amber-800"
                >
                    if concurrent {
                        "rows concurrent"
                    } else {
                        "rows sequential"
                    }
                </span>
            </div>
            <div class="overflow-x-auto">
                <table class="w-full min-w-[48rem] border-collapse text-left text-sm">
                    <thead
                        class="border-b border-slate-300 bg-slate-100 text-xs uppercase tracking-wider text-slate-500"
                    >
                        <tr>
                            <th class="px-4 py-2.5">"Item"</th>
                            <th class="px-4 py-2.5">"Specification"</th>
                            <th class="px-4 py-2.5">"Stock"</th>
                            <th class="px-4 py-2.5 text-right">"Price"</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-slate-200">
                        if concurrent {
                            for product in products {
                                product_row(
                                    product: product,
                                    mode: mode,
                                    category: category
                                )
                            }
                        } else {
                            for row in sequential_rows {
                                (row)
                            }
                        }
                    </tbody>
                </table>
            </div>
        </section>
    }
}

#[component]
async fn product_row(product: Product, mode: RenderMode, category: Category) -> Result {
    tokio::time::sleep(Duration::from_millis(product.delay_ms)).await;

    view! {
        <tr class="group hover:bg-amber-50/70">
            <td class="px-4 py-3 align-top">
                <a
                    href=(product_href(mode, category, product))
                    preload="mouseover"
                    class="font-bold text-blue-800 underline decoration-blue-800/30 underline-offset-2 group-hover:decoration-blue-800"
                >
                    (product.name)
                </a>
                <p class="mt-1 font-mono text-xs text-slate-400">
                    (product.sku)
                    " / "
                    (product.delay_ms)
                    " ms"
                </p>
            </td>
            <td class="max-w-md px-4 py-3 align-top text-slate-600">
                (product.specification)
            </td>
            <td class="px-4 py-3 align-top">
                <span class="font-bold text-emerald-700">(product.stock)</span>
                <span class="ml-1 text-xs text-slate-400">"available"</span>
            </td>
            <td class="whitespace-nowrap px-4 py-3 text-right align-top">
                <span class="font-black">(product.price)</span>
                <span class="ml-1 text-xs text-slate-400">
                    "/"
                    (product.unit)
                </span>
            </td>
        </tr>
    }
}

#[component]
async fn sourcing_notes() -> Result {
    tokio::time::sleep(Duration::from_millis(550)).await;

    view! {
        <aside
            data-arrival="sourcing"
            class="self-start rounded-lg border border-slate-300 bg-white shadow-sm"
        >
            <div class="border-b border-slate-200 bg-slate-950 px-4 py-3 text-white">
                <p class="text-xs font-bold uppercase tracking-wider text-amber-300">
                    "Sourcing desk"
                </p>
                <h2 class="mt-1 font-bold">"Need an exact match?"</h2>
            </div>
            <div class="space-y-4 p-4 text-sm leading-6 text-slate-600">
                <p>
                    "Send a drawing or specification. Our demo sourcing team will locate compatible materials and dimensions."
                </p>
                <button
                    class="w-full rounded-md bg-amber-300 px-4 py-2.5 font-bold text-slate-950 hover:bg-amber-200"
                >
                    "Request a quote"
                </button>
                <div class="border-t border-slate-200 pt-4 text-xs text-slate-500">
                    <p class="font-bold text-slate-700">"Source latency: 550 ms"</p>
                    <p>"This independent component races the product results."</p>
                </div>
            </div>
        </aside>
    }
}

#[component]
async fn product_detail(product: Product, mode: RenderMode, category: Category) -> Result {
    tokio::time::sleep(Duration::from_millis(650)).await;

    view! {
        <div data-arrival="product-detail">
            <a
                href=(catalog_href(mode, category))
                preload="mouseover"
                class="inline-flex items-center gap-2 text-sm font-bold text-blue-800 hover:underline"
            >
                <span aria-hidden="true">"<-"</span>
                "Back to "
                (category.name)
            </a>
            <div
                class="mt-4 grid overflow-hidden rounded-lg border border-slate-300 bg-white shadow-sm lg:grid-cols-[1fr_22rem]"
            >
                <section class="p-6 sm:p-8">
                    <p
                        class="font-mono text-xs font-bold uppercase tracking-wider text-slate-400"
                    >
                        (product.sku)
                    </p>
                    <h1 class="mt-2 text-3xl font-black tracking-tight">
                        (product.name)
                    </h1>
                    <p class="mt-3 max-w-2xl leading-7 text-slate-600">
                        (product.specification)
                    </p>
                    <div
                        class="mt-8 grid gap-px overflow-hidden rounded-md border border-slate-300 bg-slate-300 sm:grid-cols-3"
                    >
                        <div class="bg-slate-50 p-4">
                            <p class="text-xs font-bold uppercase text-slate-400">
                                "Material"
                            </p>
                            <p class="mt-1 font-bold">"Zinc-plated steel"</p>
                        </div>
                        <div class="bg-slate-50 p-4">
                            <p class="text-xs font-bold uppercase text-slate-400">
                                "Ships"
                            </p>
                            <p class="mt-1 font-bold text-emerald-700">"Today"</p>
                        </div>
                        <div class="bg-slate-50 p-4">
                            <p class="text-xs font-bold uppercase text-slate-400">
                                "Detail latency"
                            </p>
                            <p class="mt-1 font-bold">"650 ms"</p>
                        </div>
                    </div>
                    <div class="mt-8">
                        <h2 class="font-bold">"Technical information"</h2>
                        <dl
                            class="mt-3 divide-y divide-slate-200 border-y border-slate-200 text-sm"
                        >
                            <div class="grid grid-cols-2 py-3">
                                <dt class="text-slate-500">"Country of origin"</dt>
                                <dd class="font-semibold">"United States"</dd>
                            </div>
                            <div class="grid grid-cols-2 py-3">
                                <dt class="text-slate-500">"Compliance"</dt>
                                <dd class="font-semibold">"RoHS 3, REACH"</dd>
                            </div>
                            <div class="grid grid-cols-2 py-3">
                                <dt class="text-slate-500">"Package quantity"</dt>
                                <dd class="font-semibold">"25"</dd>
                            </div>
                        </dl>
                    </div>
                </section>
                <aside
                    class="border-t border-slate-300 bg-amber-50 p-6 lg:border-l lg:border-t-0"
                >
                    <p class="text-sm text-slate-500">
                        "Price per "
                        (product.unit)
                    </p>
                    <p class="mt-1 text-4xl font-black tracking-tight">
                        (product.price)
                    </p>
                    <p class="mt-4 text-sm">
                        <strong class="text-emerald-700">
                            (product.stock)
                            " in stock"
                        </strong>
                        " at the regional warehouse"
                    </p>
                    <label
                        for="quantity"
                        class="mt-6 block text-xs font-bold uppercase tracking-wider text-slate-500"
                    >
                        "Quantity"
                    </label>
                    <input
                        id="quantity"
                        type="number"
                        value="1"
                        min="1"
                        class="mt-2 w-full rounded-md border border-slate-400 bg-white px-3 py-2"
                    >
                    <button
                        class="mt-3 w-full rounded-md bg-slate-950 px-4 py-3 font-bold text-white hover:bg-slate-800"
                    >
                        "Add to cart"
                    </button>
                    <p class="mt-4 text-xs leading-5 text-slate-500">
                        "Hovering this product link started its 650 ms response before the click. The boosted navigation then reused the cached response."
                    </p>
                </aside>
            </div>
        </div>
    }
}

async fn inventory_placeholder() -> Result {
    view! {
        <section
            aria-busy="true"
            class="grid animate-pulse gap-px overflow-hidden rounded-lg border border-slate-300 bg-slate-300 sm:grid-cols-3"
        >
            <div class="h-24 bg-white p-5">
                <div class="h-3 w-28 rounded bg-slate-200"></div>
                <div class="mt-3 h-7 w-16 rounded bg-slate-200"></div>
            </div>
            <div class="h-24 bg-white p-5">
                <div class="h-3 w-24 rounded bg-slate-200"></div>
                <div class="mt-3 h-7 w-20 rounded bg-slate-200"></div>
            </div>
            <div class="h-24 bg-white p-5">
                <div class="h-3 w-24 rounded bg-slate-200"></div>
                <div class="mt-3 h-7 w-16 rounded bg-slate-200"></div>
            </div>
        </section>
    }
}

async fn product_table_placeholder(cx: &Cx) -> Result {
    view! {
        cx =>
        <section
            aria-busy="true"
            class="overflow-hidden rounded-lg border border-slate-300 bg-white shadow-sm"
        >
            <div class="border-b border-slate-200 bg-slate-50 p-4">
                <div class="h-4 w-48 animate-pulse rounded bg-slate-200"></div>
            </div>
            <div class="divide-y divide-slate-100 p-4">
                for _ in 0..4 {
                    <div class="grid animate-pulse grid-cols-4 gap-5 py-4">
                        <div class="h-4 rounded bg-slate-200"></div>
                        <div class="col-span-2 h-4 rounded bg-slate-100"></div>
                        <div class="h-4 rounded bg-slate-100"></div>
                    </div>
                }
            </div>
        </section>
    }
}

async fn sourcing_placeholder() -> Result {
    view! {
        <aside
            aria-busy="true"
            class="h-72 animate-pulse rounded-lg border border-slate-300 bg-white p-4 shadow-sm"
        >
            <div class="h-4 w-28 rounded bg-slate-200"></div>
            <div class="mt-4 h-7 w-48 rounded bg-slate-200"></div>
            <div class="mt-6 h-20 rounded bg-slate-100"></div>
            <div class="mt-5 h-10 rounded bg-amber-100"></div>
        </aside>
    }
}

fn request_kind(cx: &Cx) -> &'static str {
    if headers(cx).contains_key("hx-preloaded") {
        "hover preload"
    } else if hx_boosted(cx) {
        "htmx boosted"
    } else {
        "full navigation"
    }
}

fn mode_item_class(active: bool) -> &'static str {
    if active {
        "block rounded-lg bg-slate-950 px-3 py-3 text-white"
    } else {
        "block rounded-lg px-3 py-3 text-slate-700 hover:bg-slate-100"
    }
}

fn category_link_class(active: bool) -> &'static str {
    if active {
        "flex items-center justify-between rounded-md bg-amber-100 px-3 py-2.5 text-sm text-slate-950 ring-1 ring-amber-300"
    } else {
        "flex items-center justify-between rounded-md px-3 py-2.5 text-sm text-slate-700 hover:bg-slate-100"
    }
}

fn mode_href(mode: RenderMode, category: Category) -> String {
    format!("/?mode={}&category={}", mode.slug(), category.slug)
}

fn catalog_href(mode: RenderMode, category: Category) -> String {
    mode_href(mode, category)
}

fn product_suggestions(mode: RenderMode) -> Vec<ProductSuggestion> {
    products()
        .into_iter()
        .map(|product| ProductSuggestion {
            name: product.name,
            sku: product.sku,
            specification: product.specification,
            href: product_href(mode, find_category(Some(product.category)), product),
        })
        .collect()
}

fn product_href(mode: RenderMode, category: Category, product: Product) -> String {
    format!(
        "/?mode={}&category={}&product={}",
        mode.slug(),
        category.slug,
        product.sku
    )
}

fn all_category() -> Category {
    categories()[0]
}

fn find_category(slug: Option<&str>) -> Category {
    slug.and_then(|slug| {
        categories()
            .into_iter()
            .find(|category| category.slug == slug)
    })
    .unwrap_or(categories()[1])
}

fn find_product(sku: &str) -> Option<Product> {
    products().into_iter().find(|product| product.sku == sku)
}

fn matching_products(category: Category, query: Option<&str>) -> Vec<Product> {
    let query = query.map(str::to_ascii_lowercase);
    products()
        .into_iter()
        .filter(|product| category.slug == "all" || product.category == category.slug)
        .filter(|product| {
            query.as_ref().is_none_or(|query| {
                product.name.to_ascii_lowercase().contains(query)
                    || product.specification.to_ascii_lowercase().contains(query)
                    || product.sku.to_ascii_lowercase().contains(query)
            })
        })
        .collect()
}

fn categories() -> [Category; 5] {
    [
        Category {
            slug: "all",
            name: "All Products",
            description: "Search the complete stocked catalog across mechanical, electrical, material handling, and safety supplies.",
            count: 12,
        },
        Category {
            slug: "fasteners",
            name: "Fasteners",
            description: "Bolts, screws, nuts, washers, and threaded hardware for production and maintenance work.",
            count: 4,
        },
        Category {
            slug: "electrical",
            name: "Electrical",
            description: "Industrial connectors, enclosures, cable management, and control components.",
            count: 3,
        },
        Category {
            slug: "material-handling",
            name: "Material Handling",
            description: "Casters, carts, lifting hardware, and warehouse equipment for moving heavy loads.",
            count: 3,
        },
        Category {
            slug: "safety",
            name: "Safety",
            description: "Protective equipment and facility controls for safer industrial operations.",
            count: 2,
        },
    ]
}

fn products() -> [Product; 12] {
    [
        Product {
            category: "fasteners",
            sku: "91257A540",
            name: "Grade 8 Hex Head Screw",
            specification: "Zinc-plated steel, 3/8\"-16 thread, 2\" long",
            price: "$12.80",
            unit: "pack",
            stock: 184,
            delay_ms: 160,
        },
        Product {
            category: "fasteners",
            sku: "90322A194",
            name: "Nylon-Insert Locknut",
            specification: "Zinc-plated steel, 3/8\"-16 thread size",
            price: "$8.45",
            unit: "pack",
            stock: 326,
            delay_ms: 220,
        },
        Product {
            category: "fasteners",
            sku: "92141A029",
            name: "Oversized Flat Washer",
            specification: "18-8 stainless steel, 3/8\" screw size",
            price: "$6.18",
            unit: "pack",
            stock: 91,
            delay_ms: 280,
        },
        Product {
            category: "fasteners",
            sku: "92383A824",
            name: "Thread-Locking Socket Screw",
            specification: "Alloy steel, 1/4\"-20 thread, 1-1/4\" long",
            price: "$15.62",
            unit: "pack",
            stock: 74,
            delay_ms: 190,
        },
        Product {
            category: "electrical",
            sku: "7566K42",
            name: "Watertight Cord Grip",
            specification: "Nylon body, 1/2 NPT, 0.24\"-0.47\" cable",
            price: "$7.92",
            unit: "each",
            stock: 208,
            delay_ms: 170,
        },
        Product {
            category: "electrical",
            sku: "7304K114",
            name: "DIN-Rail Terminal Block",
            specification: "600 V AC/DC, 30 A, 10 AWG maximum wire",
            price: "$24.10",
            unit: "pack",
            stock: 147,
            delay_ms: 240,
        },
        Product {
            category: "electrical",
            sku: "7727K18",
            name: "Polycarbonate Enclosure",
            specification: "NEMA 4X, 8\" x 6\" x 4\", clear cover",
            price: "$38.75",
            unit: "each",
            stock: 63,
            delay_ms: 210,
        },
        Product {
            category: "material-handling",
            sku: "2415T38",
            name: "Swivel Plate Caster",
            specification: "Polyurethane wheel, 5\" diameter, 700 lb capacity",
            price: "$42.60",
            unit: "each",
            stock: 118,
            delay_ms: 180,
        },
        Product {
            category: "material-handling",
            sku: "2492T17",
            name: "Low-Profile Platform Truck",
            specification: "Steel deck, 2,000 lb capacity, 48\" x 24\"",
            price: "$486.00",
            unit: "each",
            stock: 12,
            delay_ms: 260,
        },
        Product {
            category: "material-handling",
            sku: "3031T22",
            name: "Web Sling",
            specification: "Eye-and-eye, 2\" wide, 6 ft long, 3,200 lb capacity",
            price: "$31.40",
            unit: "each",
            stock: 86,
            delay_ms: 200,
        },
        Product {
            category: "safety",
            sku: "5450T14",
            name: "Anti-Fog Safety Glasses",
            specification: "Clear polycarbonate lens, ANSI Z87.1+",
            price: "$9.85",
            unit: "pair",
            stock: 412,
            delay_ms: 150,
        },
        Product {
            category: "safety",
            sku: "5924T31",
            name: "Lockout Station",
            specification: "Wall mount, stocked for 10 workers",
            price: "$214.00",
            unit: "each",
            stock: 28,
            delay_ms: 230,
        },
    ]
}

#[cfg(test)]
mod tests {
    use topcoat::{
        asset::{AssetConfig, Manifest},
        context::CxTestBuilder,
    };

    use super::*;

    #[tokio::test]
    async fn typeahead_server_render_is_deferred() {
        let manifest = Manifest::parse(&format!(
            r#"
version = 1

[[assets]]
id = {}
file = "typeahead.js"
hash = "0"
content_type = "text/javascript"
"#,
            TYPEAHEAD_JS.id().as_u64()
        ))
        .unwrap();
        let cx = CxTestBuilder::new()
            .app_context(AssetConfig::hosted_at("https://example.com", manifest))
            .build();
        let view = deferred_typeahead(&cx, RenderMode::Streaming, "valve")
            .await
            .unwrap();
        let rendered = view.render_response(&cx);

        assert!(rendered.html.contains("data-topcoat-defer-start"));
        assert!(rendered.html.contains("value=\"valve\""));
        assert!(!rendered.html.contains("data-topcoat-react-ssr"));
        assert_eq!(rendered.deferred.len(), 1);

        let completed = rendered.deferred[0]
            .clone()
            .resolve(cx.handle())
            .await
            .unwrap();
        let html = completed.render(&cx);

        assert!(html.contains("data-topcoat-react-ssr"), "{html}");
        assert!(html.contains("class=\"contents\""), "{html}");
        assert!(html.contains("<input"), "{html}");
        assert!(html.contains("Search by product"), "{html}");
    }
}
