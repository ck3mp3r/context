export class Calculator {
  private value: number;

  constructor(initial: number) {
    this.value = initial;
  }

  add(n: number): Calculator {
    this.value += n;
    return this;
  }

  getResult(): number {
    return this.value;
  }
}

export namespace MathUtils {
  export function square(x: number): number {
    return x * x;
  }

  export const PI = 3.14159;
}
