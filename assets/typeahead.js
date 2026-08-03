const selector = "input[data-topcoat-typeahead]";
const cleanups = new Map();
let nextListId = 0;

function initialize(input) {
  if (input.dataset.topcoatTypeaheadReady) return;
  input.dataset.topcoatTypeaheadReady = "true";

  const list = document.createElement("div");
  const listId = `catalog-typeahead-${nextListId++}`;
  list.id = listId;
  list.className = "topcoat-typeahead";
  list.role = "listbox";
  list.hidden = true;
  document.body.append(list);
  input.setAttribute("aria-controls", listId);

  let products = [];
  let matches = [];
  let activeIndex = -1;

  function position() {
    if (list.hidden || !input.isConnected) return;
    const bounds = input.getBoundingClientRect();
    list.style.left = `${bounds.left}px`;
    list.style.top = `${bounds.bottom + 4}px`;
    list.style.width = `${bounds.width}px`;
  }

  function close() {
    list.hidden = true;
    input.setAttribute("aria-expanded", "false");
    input.removeAttribute("aria-activedescendant");
    activeIndex = -1;
  }

  function select(index) {
    const options = [...list.children];
    if (!options.length) return;
    activeIndex = (index + options.length) % options.length;
    options.forEach((option, optionIndex) => {
      option.setAttribute("aria-selected", String(optionIndex === activeIndex));
    });
    input.setAttribute("aria-activedescendant", options[activeIndex].id);
    options[activeIndex].scrollIntoView({ block: "nearest" });
  }

  function render() {
    const query = input.value.trim().toLowerCase();
    list.replaceChildren();
    if (!query) {
      close();
      return;
    }

    const terms = query.split(/\s+/);
    matches = products
      .filter((product) => {
        const searchable = `${product.name} ${product.sku} ${product.specification}`.toLowerCase();
        return terms.every((term) => searchable.includes(term));
      })
      .slice(0, 8);

    for (const [index, product] of matches.entries()) {
      const option = document.createElement("a");
      option.id = `${listId}-${index}`;
      option.className = "topcoat-typeahead__item";
      option.href = product.href;
      option.role = "option";
      option.setAttribute("aria-selected", "false");

      const heading = document.createElement("span");
      heading.className = "topcoat-typeahead__heading";
      const name = document.createElement("span");
      name.textContent = product.name;
      const sku = document.createElement("span");
      sku.className = "topcoat-typeahead__sku";
      sku.textContent = product.sku;
      heading.append(name, sku);

      const specification = document.createElement("span");
      specification.className = "topcoat-typeahead__specification";
      specification.textContent = product.specification;
      option.append(heading, specification);
      list.append(option);
    }

    if (!matches.length) {
      close();
      return;
    }
    list.hidden = false;
    input.setAttribute("aria-expanded", "true");
    activeIndex = -1;
    position();
  }

  function handleKeydown(event) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      select(activeIndex + 1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      select(activeIndex - 1);
    } else if (event.key === "Enter" && activeIndex >= 0) {
      event.preventDefault();
      list.children[activeIndex].click();
    } else if (event.key === "Escape") {
      close();
    }
  }

  function handleBlur() {
    setTimeout(close, 100);
  }

  input.addEventListener("input", render);
  input.addEventListener("focus", render);
  input.addEventListener("keydown", handleKeydown);
  input.addEventListener("blur", handleBlur);
  list.addEventListener("mousedown", (event) => event.preventDefault());
  window.addEventListener("resize", position);
  window.addEventListener("scroll", position, true);
  cleanups.set(input, () => {
    list.remove();
    window.removeEventListener("resize", position);
    window.removeEventListener("scroll", position, true);
    cleanups.delete(input);
  });

  globalThis.topcoat.json(input.dataset.topcoatTypeahead).then((value) => {
    if (!input.isConnected) return;
    products = value;
    render();
  });
}

function scan(root) {
  if (root instanceof Element && root.matches(selector)) initialize(root);
  root.querySelectorAll?.(selector).forEach(initialize);
}

scan(document);
new MutationObserver((records) => {
  for (const [input, cleanup] of cleanups) {
    if (!input.isConnected) cleanup();
  }
  for (const record of records) record.addedNodes.forEach(scan);
}).observe(document.documentElement, { childList: true, subtree: true });
