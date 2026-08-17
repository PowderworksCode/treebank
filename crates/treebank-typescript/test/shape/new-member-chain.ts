// `new` takes a member expression, never a call: this is
// `(new Date()).getFullYear()`, not `new (Date().getFullYear())`.
export const year = new Date().getFullYear();
export const nested = new a.b.C();
export const parenthesised = new (f())();
export const chained = new C().m().n;
export const subscripted = new arr[0]();
