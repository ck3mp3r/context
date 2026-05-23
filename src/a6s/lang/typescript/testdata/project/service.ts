import { User, UserRepository } from './user';

export class UserService {
  private repo: UserRepository;

  constructor(repo: UserRepository) {
    this.repo = repo;
  }

  async getUser(id: string): Promise<User | null> {
    return this.repo.findById(id);
  }

  async createUser(name: string, email: string): Promise<User> {
    const user: User = {
      id: crypto.randomUUID(),
      name,
      email,
      createdAt: new Date(),
    };
    return this.repo.save(user);
  }
}
