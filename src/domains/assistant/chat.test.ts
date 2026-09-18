import { describe, expect, test } from "vitest";
import { toEvent } from "./chat.store.svelte";

describe("toEvent", () => {
  test("normalizes known statuses, defaults everything else to running", () => {
    expect(toEvent({ id: "1", title: "a", status: "done" })).toEqual({
      id: "1",
      title: "a",
      status: "done",
      continuous: false,
    });
    expect(toEvent({ id: "2", title: "b", status: "failed" })).toEqual({
      id: "2",
      title: "b",
      status: "failed",
      continuous: false,
    });
    const running = toEvent({ id: "3", title: "c", status: "running" });
    expect(running).toMatchObject({ status: "running", continuous: true });
    expect(toEvent({ id: "4", title: "d", status: "weird" })).toMatchObject({
      status: "running",
      continuous: true,
    });
  });
});
