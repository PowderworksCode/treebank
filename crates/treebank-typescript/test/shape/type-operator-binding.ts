// `readonly` and `keyof` bind tighter than `|` and `&`:
// `(readonly string[]) | undefined`, not `readonly (string[] | undefined)`.
export type A = readonly string[] | undefined;
export type B = keyof C | D;
export type E = readonly (C | D)[] & { tag: 1 };
type C = { c: 1 };
type D = { d: 2 };
