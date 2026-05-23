import { Entity, Repository } from './models';

export interface User extends Entity {
  name: string;
  email: string;
}

export class UserRepository implements Repository<User> {
  private store: Map<string, User> = new Map();

  async findById(id: string): Promise<User | null> {
    return this.store.get(id) ?? null;
  }

  async save(entity: User): Promise<User> {
    this.store.set(entity.id, entity);
    return entity;
  }

  async delete(id: string): Promise<void> {
    this.store.delete(id);
  }
}
