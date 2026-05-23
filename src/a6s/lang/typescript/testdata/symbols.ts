export class UserService {
  private name: string;
  readonly id: number;

  constructor(name: string, id: number) {
    this.name = name;
    this.id = id;
  }

  getName(): string {
    return this.name;
  }

  static create(name: string): UserService {
    return new UserService(name, 0);
  }
}

export abstract class BaseEntity {
  abstract getId(): string;
}

export interface Repository<T> {
  findById(id: string): T | null;
  save(entity: T): void;
  delete(id: string): boolean;
}

export type UserId = string;
type InternalConfig = { debug: boolean };

export enum Status {
  Active = "active",
  Inactive = "inactive",
  Pending = "pending",
}

export function createUser(name: string, age: number): UserService {
  return UserService.create(name);
}

function internalHelper(): void {
  console.log("helper");
}

export const MAX_USERS = 100;
const SECRET_KEY = "abc123";
export let mutableCount = 0;
