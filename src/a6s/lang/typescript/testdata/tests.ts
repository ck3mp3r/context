import { describe, it, expect } from 'vitest';
import { UserService } from './symbols';

describe('UserService', () => {
  it('should create a user', () => {
    const service = UserService.create('test');
    expect(service).toBeDefined();
  });

  it('should return name', () => {
    const service = new UserService('test', 1);
    expect(service.getName()).toBe('test');
  });
});

export function testHelper(): void {
  // helper for tests
}
