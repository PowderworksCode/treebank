// The type after `as` is greedy: across `&`, across `|`, and across `.`.
declare const x: unknown;
declare namespace React { type ReactElement = unknown; }
export const a = x as A & B;
export const b = x as A | B;
export const c = x as React.ReactElement;
export const d = x as A & { c?: B };
type A = { a: 1 };
type B = { b: 2 };
