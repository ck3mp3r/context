export interface Serializable {
  serialize(): string;
}

export interface Identifiable {
  getId(): string;
}

export class BaseModel implements Serializable {
  serialize(): string {
    return JSON.stringify(this);
  }
}

export class User extends BaseModel implements Identifiable {
  private id: string;
  name: string;

  constructor(id: string, name: string) {
    super();
    this.id = id;
    this.name = name;
  }

  getId(): string {
    return this.id;
  }
}

export interface ReadonlyRepository<T> extends Identifiable {
  findAll(): T[];
}
