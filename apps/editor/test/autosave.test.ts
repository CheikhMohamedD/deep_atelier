import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { Autosave, type SaveStatus } from "@/lib/editor/autosave";

class Conflict extends Error {}

function setup(save: (base: number) => Promise<number>) {
  const statuses: SaveStatus[] = [];
  const autosave = new Autosave(save, 1, {
    onStatus: (status) => statuses.push(status),
    isConflict: (error) => error instanceof Conflict,
    debounceMs: 100,
    retryMs: 1000,
  });
  return { autosave, statuses };
}

beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
});

describe("autosave", () => {
  it("saves once after a burst of changes, from the last version", async () => {
    const save = vi.fn(async (base: number) => base + 1);
    const { autosave, statuses } = setup(save);
    autosave.changed();
    autosave.changed();
    autosave.changed();
    expect(autosave.unsaved).toBe(true);
    await vi.advanceTimersByTimeAsync(100);
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith(1);
    expect(autosave.version).toBe(2);
    expect(statuses).toEqual(["dirty", "saving", "saved"]);
    expect(autosave.unsaved).toBe(false);
  });

  it("saves again when the document changes during a save", async () => {
    let release: (() => void) | undefined;
    const save = vi.fn(
      (base: number) =>
        new Promise<number>((resolve) => {
          release = () => resolve(base + 1);
        }),
    );
    const { autosave } = setup(save);
    autosave.changed();
    await vi.advanceTimersByTimeAsync(100);
    autosave.changed();
    release?.();
    await vi.advanceTimersByTimeAsync(0);
    expect(autosave.status).toBe("dirty");
    await vi.advanceTimersByTimeAsync(100);
    expect(save).toHaveBeenCalledTimes(2);
    expect(save).toHaveBeenLastCalledWith(2);
    release?.();
    await vi.advanceTimersByTimeAsync(0);
    expect(autosave.status).toBe("saved");
  });

  it("stops on a version conflict and retries after other errors", async () => {
    const conflicting = setup(async () => {
      throw new Conflict("409");
    });
    conflicting.autosave.changed();
    await vi.advanceTimersByTimeAsync(100);
    expect(conflicting.autosave.status).toBe("conflict");
    conflicting.autosave.changed();
    await vi.advanceTimersByTimeAsync(10_000);
    expect(conflicting.autosave.status).toBe("conflict");

    let fail = true;
    const save = vi.fn(async (base: number) => {
      if (fail) throw new Error("network");
      return base + 1;
    });
    const flaky = setup(save);
    flaky.autosave.changed();
    await vi.advanceTimersByTimeAsync(100);
    expect(flaky.autosave.status).toBe("error");
    fail = false;
    await vi.advanceTimersByTimeAsync(1000);
    expect(save).toHaveBeenCalledTimes(2);
    expect(flaky.autosave.status).toBe("saved");
    expect(flaky.autosave.version).toBe(2);
  });
});
