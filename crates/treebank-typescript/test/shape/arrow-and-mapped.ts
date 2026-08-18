// A `{` after `=>` opens a BLOCK. An object literal there needs parentheses.
declare let notifyFn: unknown;
export const setNotify = (fn: unknown) => {
  notifyFn = fn;
};
export const makeObj = () => ({ a: 1 });

// A mapped type is a mapped type in annotation position too, not an object
// type whose property name is an `in` expression.
type N = "a" | "b";
export declare const signalsByNumber: {
  [K in N]: string
};
export type Mapped = { [K in N]?: string };
