import { IndexedDBIdentityStore, getOrCreateIdentity } from '@the9ines/bolt-transport-web';
import type { IdentityKeyPair } from '@the9ines/bolt-core';

const identityStore = new IndexedDBIdentityStore();

let cachedIdentity: IdentityKeyPair | null = null;

export async function initIdentity(): Promise<IdentityKeyPair> {
  if (cachedIdentity) return cachedIdentity;
  cachedIdentity = await getOrCreateIdentity(identityStore);
  return cachedIdentity;
}
