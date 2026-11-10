import { expect, test } from "vitest";
import { itemAt, median } from "./arrays";

test("item at", () => {
  expect(itemAt(["a", "b"], 1)).toBe("b");
  expect(() => itemAt(["a"], 1)).toThrow(RangeError);
});

test("median", () => {
  expect(median([5])).toBe(5);
  expect(median([3, 1, 2])).toBe(2);
  expect(median([4, 1, 3, 2])).toBe(2.5);
});
