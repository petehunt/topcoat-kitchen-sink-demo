import React from "react";
import { createRoot, hydrateRoot } from "react-dom/client";
import { SWRConfig } from "swr";

import { Typeahead, type TypeaheadProps } from "./typeahead-component";

type MountContext<Props> = {
  element: HTMLElement;
  props: Props;
  fallback: Record<string, unknown>;
  serverRendered: boolean;
};

declare global {
  interface Window {
    topcoat: {
      react: {
        register<Props>(
          name: string,
          mount: (context: MountContext<Props>) => void | (() => void),
        ): void;
      };
    };
  }
}

window.topcoat.react.register<TypeaheadProps>(
  "catalog-typeahead",
  ({ element, props, fallback, serverRendered }) => {
    const app = (
      <SWRConfig value={{ fallback }}>
        <Typeahead {...props} />
      </SWRConfig>
    );
    const root = serverRendered ? hydrateRoot(element, app) : createRoot(element);
    if (!serverRendered) root.render(app);
    return () => root.unmount();
  },
);
