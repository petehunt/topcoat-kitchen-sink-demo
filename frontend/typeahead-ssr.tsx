import "./quickjs-polyfills";

import React from "react";
import { renderToString } from "react-dom/server.browser";
import { SWRConfig } from "swr";

import { Typeahead, type TypeaheadProps } from "./typeahead-component";

globalThis.topcoatReactRender = (
  props: TypeaheadProps,
  fallback: Record<string, unknown>,
) =>
  renderToString(
    <SWRConfig value={{ fallback }}>
      <Typeahead {...props} />
    </SWRConfig>,
  );

declare global {
  var topcoatReactRender: (
    props: TypeaheadProps,
    fallback: Record<string, unknown>,
  ) => string;
}
