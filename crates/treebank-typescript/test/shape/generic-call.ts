// A generic call on a member function is a call, not a comparison chain.
declare const vi: { importActual<T>(s: string): Promise<T> };
declare const z: { custom<T>(f: (v: unknown) => boolean): T };
export const a = vi.importActual<any>("valibot");
export const b = z.custom<string>((val) => typeof val === "string");
// ...and an ordinary comparison is still a comparison.
declare const x: number, y: number, w: number;
export const c = x < y > w;

// A three-level qualified type name after `as` is a type.
declare namespace n { namespace core { type S = string } }
export const d = (a as unknown) as n.core.S;

// `new X!(a)` asserts the constructor; the arguments belong to `new`.
declare const Ctor: (new (u: string) => object) | undefined;
export const e = new Ctor!("u");
