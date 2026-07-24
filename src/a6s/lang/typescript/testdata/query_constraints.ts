// Fixture for constrained query tests (c5t note 7d1244e0, Task 1).
// Every "should NOT" case is a regression guard for the bare-query bugs.

// === Top-level declarations (SHOULD be extracted) ===
export class TopClass {
  method(): void {}
  field: number = 0;
}

class InternalClass {
  helper(): string { return "x"; }
}

export interface TopInterface {
  ifaceMethod(x: string): boolean;
}

export type TopType = string;
type InternalType = { debug: boolean };

export enum TopEnum {
  A = "a",
  B = "b",
}

export function topFunction(): void {}
function internalFunction(): void {}

export const TOP_CONST = 42;
const INTERNAL_CONST = "secret";
export let TOP_LET = 0;

// === Object-literal methods (should NOT be extracted) ===
// The headline bug: bare method_definition query matched these.
const obj = {
  objMethod(): void {},
  objMethod2() { return 1; },
};

const handler = {
  handle(req: string): void {},
};

// === Local variables (should NOT be extracted) ===
// The is_top_level band-aid fixed these; now the query handles it.
function withLocals() {
  const localConst = 1;
  let localLet = 2;
  var localVar = 3;
}

// === Nested declarations (should NOT be extracted as top-level) ===
function withNested() {
  function nestedFn(): void {}
  class NestedClass {}
  interface NestedIface { x: number; }
  enum NestedEnum { X, Y }
  type NestedType = string;
}

// === Nested class with methods (should NOT be extracted) ===
function withNestedClass() {
  class NestedClassWithMethods {
    nestedMethod(): void {}
    nestedField: number = 0;
  }
}

// === Inline object type method signatures (should NOT be extracted) ===
// is_inside_inline_object_type band-aid fixed these; now the query handles it.
export type WithInline = {
  inlineMethod(): void;
  cleanup?(): void;
};

export type NestedInline = {
  inner: {
    process(): void;
  };
};

// === Class-body method signature overloads (SHOULD be extracted as interface_method) ===
class WithOverload {
  method(x: string): string;
  method(x: number): number;
  method(x: any): any { return x; }
}
