export interface Logger {
  log(message: string): void;
}

export class Service {
  private logger: Logger;
  private config: Map<string, string>;

  constructor(logger: Logger) {
    this.logger = logger;
  }

  process(input: Request): Response {
    this.logger.log("processing");
    return new Response();
  }

  getItems(): Array<Item> {
    return [];
  }
}

export function transform(data: Buffer, encoder: TextEncoder): Uint8Array {
  return encoder.encode(data.toString());
}

export type Handler = (req: Request, res: Response) => void;

export class Request {}
export class Response {}
export class Item {}
