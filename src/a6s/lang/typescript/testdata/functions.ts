export const fetchData = async (url: string): Promise<Response> => {
  return fetch(url);
};

export const add = (a: number, b: number): number => a + b;

export function* generateIds(): Generator<number> {
  let id = 0;
  while (true) yield id++;
}

export async function loadConfig(path: string): Promise<Config> {
  const data = await readFile(path);
  return JSON.parse(data);
}

interface Config {
  port: number;
  host: string;
}
