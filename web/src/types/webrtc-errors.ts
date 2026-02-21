import { BoltError } from '@the9ines/bolt-core';

export { BoltError as WebRTCError, ConnectionError, TransferError, EncryptionError } from '@the9ines/bolt-core';

export class SignalingError extends BoltError {
  constructor(message: string, details?: unknown) {
    super(message, details);
    this.name = 'SignalingError';
  }
}
