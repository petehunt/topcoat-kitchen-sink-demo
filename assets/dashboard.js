(() => {
  const stamp = (root) => {
    const nodes = [];
    if (root.nodeType === Node.ELEMENT_NODE && root.matches("[data-arrival]")) {
      nodes.push(root);
    }
    if (root.querySelectorAll) {
      nodes.push(...root.querySelectorAll("[data-arrival]"));
    }

    for (const node of nodes) {
      if (node.dataset.arrivalStamped) continue;
      node.dataset.arrivalStamped = "true";
      const output = node.querySelector("[data-arrival-time]");
      if (output) output.textContent = `${Math.round(performance.now())} ms`;
    }
  };

  new MutationObserver((records) => {
    for (const record of records) {
      for (const node of record.addedNodes) stamp(node);
    }
  }).observe(document.documentElement, { childList: true, subtree: true });
})();
