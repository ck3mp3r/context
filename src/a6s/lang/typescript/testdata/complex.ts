@Injectable()
export class AuthService {
  @Inject()
  private readonly logger: Logger;

  @LogCall()
  async authenticate(token: string): Promise<User | null> {
    return null;
  }
}

export function Inject(): PropertyDecorator {
  return () => {};
}

export function Injectable(): ClassDecorator {
  return () => {};
}

export function LogCall(): MethodDecorator {
  return () => {};
}

interface Logger {
  log(msg: string): void;
}

interface User {
  id: string;
}
