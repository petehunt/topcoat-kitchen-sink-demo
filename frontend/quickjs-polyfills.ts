if (typeof globalThis.MessageChannel === "undefined") {
  class QuickJsMessageChannel {
    port1 = { onmessage: null } as MessagePort;
    port2 = {
      postMessage: (data: unknown) => this.port1.onmessage?.({ data } as MessageEvent),
    } as MessagePort;
  }

  Object.defineProperty(globalThis, "MessageChannel", {
    configurable: true,
    value: QuickJsMessageChannel,
  });
}

if (typeof globalThis.TextEncoder === "undefined") {
  class QuickJsTextEncoder {
    readonly encoding = "utf-8";

    encode(input = "") {
      const bytes: number[] = [];
      for (const character of input) {
        const point = character.codePointAt(0)!;
        if (point < 0x80) bytes.push(point);
        else if (point < 0x800) {
          bytes.push(0xc0 | (point >> 6), 0x80 | (point & 0x3f));
        } else if (point < 0x10000) {
          bytes.push(
            0xe0 | (point >> 12),
            0x80 | ((point >> 6) & 0x3f),
            0x80 | (point & 0x3f),
          );
        } else {
          bytes.push(
            0xf0 | (point >> 18),
            0x80 | ((point >> 12) & 0x3f),
            0x80 | ((point >> 6) & 0x3f),
            0x80 | (point & 0x3f),
          );
        }
      }
      return Uint8Array.from(bytes);
    }
  }

  Object.defineProperty(globalThis, "TextEncoder", {
    configurable: true,
    value: QuickJsTextEncoder,
  });
}
