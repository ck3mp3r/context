// This file has intentional syntax errors for edge case testing
export class Broken {
  // Missing closing brace for method
  process(data: string): void {
    console.log(data
  }
}
