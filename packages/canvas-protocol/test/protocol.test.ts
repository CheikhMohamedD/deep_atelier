import { describe, expect, it } from "vitest";

import { CHANNEL, readFrameMessage, readHostMessage, wrap } from "../src/index";

describe("canvas protocol", () => {
  it("round-trips well-formed messages in their envelope", () => {
    const rects = wrap({
      type: "rects",
      rects: { "n_a/n_b": { x: 0, y: 10, width: 390, height: 48 } },
    });
    expect(rects.channel).toBe(CHANNEL);
    expect(readFrameMessage(rects)).toEqual(rects.message);
    expect(
      readFrameMessage(wrap({ type: "layout", height: 1200, overflow: false })),
    ).toEqual({ type: "layout", height: 1200, overflow: false });
    expect(readFrameMessage(wrap({ type: "hit", id: 3, key: null }))).toEqual({
      type: "hit",
      id: 3,
      key: null,
    });
    expect(
      readHostMessage(wrap({ type: "hit_test", id: 1, x: 12, y: 40 })),
    ).toEqual({ type: "hit_test", id: 1, x: 12, y: 40 });
    expect(readHostMessage(wrap({ type: "mode", mode: "preview" }))).toEqual({
      type: "mode",
      mode: "preview",
    });
  });

  it("ignores messages from other channels or with a wrong shape", () => {
    expect(readFrameMessage({ type: "ready" })).toBeNull();
    expect(
      readFrameMessage({ channel: "other", message: { type: "ready" } }),
    ).toBeNull();
    expect(
      readFrameMessage(
        wrap({ type: "layout", height: -1, overflow: false } as never),
      ),
    ).toBeNull();
    expect(
      readFrameMessage({
        channel: CHANNEL,
        message: {
          type: "rects",
          rects: { a: { x: 0, y: 0, width: "10", height: 2 } },
        },
      }),
    ).toBeNull();
    expect(
      readFrameMessage({
        channel: CHANNEL,
        message: { type: "navigate", page: "" },
      }),
    ).toBeNull();
    expect(
      readHostMessage({
        channel: CHANNEL,
        message: { type: "mode", mode: "edit" },
      }),
    ).toBeNull();
    expect(
      readHostMessage({
        channel: CHANNEL,
        message: { type: "render", page: {} },
      }),
    ).toBeNull();
    expect(
      readHostMessage({
        channel: CHANNEL,
        message: { type: "hit_test", id: 1, x: NaN, y: 0 },
      }),
    ).toBeNull();
    expect(readHostMessage(null)).toBeNull();
  });
});
