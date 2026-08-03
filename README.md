# Kitchen sink demo

This example is a McMaster-Carr-inspired product catalog that puts Topcoat's rendering and navigation features in one app. The same catalog can be loaded with sequential rendering, concurrent server rendering, or streaming deferred views, making the differences easy to compare without changing the data or page structure.

It also demonstrates boosted page transitions with htmx, hover preloading, automatically concurrent component loops, nested deferred views, both forms of deferred content, content-hashed static assets, and Tailwind CSS.

## Run the example

Install the local Topcoat CLI from the repository root if needed:

```sh
cargo install --path crates/topcoat-cli --locked
```

Then start the development server:

```sh
cd ../kitchen-sink-demo
topcoat dev
```

Open `http://127.0.0.1:3000/`.

## Compare rendering modes

Open the menu in the dark utility bar to choose a mode. Mode changes use a full navigation so the browser exposes each strategy faithfully.

- **Sequential** awaits the inventory summary, every visible product row, and the sourcing panel in order.
- **Concurrent SSR** starts independent page components and product rows together while preserving source order.
- **Deferred View** sends the catalog chrome and placeholders immediately, then streams ordinary completed views into their annotated slots. Inventory and sourcing use `View::defer`; the product table uses inline `defer`, and all of them compose without a separate container type or include helper.

Every component has an artificial delay. The server timing and source-latency labels make it possible to verify that the modes schedule the same work differently.

## Try fast navigation

Category, search, product-detail, and back links inherit `hx-boost="true"`, so they update the document body and browser history without a full reload. Category and product links also use `preload="mouseover"`; resting the pointer on one for 100 ms begins its cacheable request before the click.

The utility bar labels the response as a normal navigation, an htmx-boosted request, or a hover preload. Product details deliberately take 650 ms to render, making the preloaded and non-preloaded paths easy to distinguish in the browser's network panel.

The example serves htmx 2.0.10 and the preload extension 2.1.2 as one minified, content-hashed script. Its logo, local script, and generated Tailwind stylesheet use the same Topcoat asset bundle.

## Deploy to Vercel

The repository includes a Rust function at `api/index.rs`. Its Vercel build command installs the pinned Topcoat CLI, builds the release function, generates the asset bundle, and includes that bundle with the function.

Link and deploy from this repository root:

```sh
vercel link
vercel deploy
```
