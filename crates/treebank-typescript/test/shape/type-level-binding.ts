// An infer constraint is greedy, and the inferred name plus its constraint
// is one `type_parameter` node.
export type Head<T> = T extends [infer L extends P, ...infer R extends P[]] ? L : never;
export type Elem<T> = T extends (infer U extends N | null)[] ? U : never;

// A function type's return type is greedy: the conditional is the RETURN,
// not something the function type sits inside.
export type Eq<T, U> = (<X>() => X extends T ? 1 : 2) extends (<X>() => X extends U ? 1 : 2) ? true : false;

// ...but the `?` of an enclosing conditional still binds outside.
export type Outer<T> = T extends () => X ? 1 : 2;

type P = { p: 1 };
type N = { n: 1 };
type X = { x: 1 };
