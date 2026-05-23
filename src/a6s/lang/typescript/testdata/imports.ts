import { UserService, type UserId } from './symbols';
import { Calculator as Calc } from './edges';
import * as MathUtils from './edges';
import DefaultExport from './default-module';
import './side-effect-module';
import type { Logger } from './type_refs';

export { UserService } from './symbols';
export * from './edges';
export { default as RenamedDefault } from './default-module';

export function processUser(id: UserId): UserService {
  const calc = new Calc(0);
  return new UserService('test', 0);
}
