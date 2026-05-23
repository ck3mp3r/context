// This file tests that inline type annotation members are NOT extracted as symbols.
// The `dispose` method signature appears inside an inline object type, not an interface body.

export type ComponentFactory<Component> = (
  container: Container
) => (Component & { dispose?(): void }) | Promise<Component & { dispose?(): void }>;

// This interface method SHOULD be extracted
export interface Disposable {
  dispose(): void;
}

// This class method SHOULD be extracted
export class Resource implements Disposable {
  dispose(): void {
    // cleanup
  }
}

// Nested object types with method signatures — should NOT be extracted
export type ComplexType = {
  handler: {
    process(): void;
    cleanup(): void;
  };
};

// Property signatures inside inline types — should NOT be extracted
export type Config = {
  name: string;
  value: number;
};

export interface Container {
  get(id: string): unknown;
}
