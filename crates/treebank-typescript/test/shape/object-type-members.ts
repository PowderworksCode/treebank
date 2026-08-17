// Members are separated, not juxtaposed. `readonly` is a legal property
// name, so with juxtaposition allowed `readonly maxSize: number` had a
// second reading as two members and sometimes took it.
export interface Cache {
  readonly maxAge?: number;

  /** doc comment between members */
  readonly maxSize: number;
}
export interface Overloads {
  (data: object): Node;
  (data: object[]): Node[];
}
declare class Node {}
