// Labelled tuple elements are nodes, so the tree says which type each
// label belongs to. The `...` goes inside the rest form.
export type Args = [opts?: Options | undefined, ...rest: Extra[]];
type Options = { o: 1 };
type Extra = { e: 1 };
