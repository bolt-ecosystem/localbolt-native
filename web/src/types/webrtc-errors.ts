
export class WebRTCError extends Error {
  constructor(message: string, public details?: any) {
    super(message);
    this.name = 'WebRTCError';
  }
}

export class ConnectionError extends WebRTCError {
  constructor(message: string, details?: any) {
    super(message, details);
    this.name = 'ConnectionError';
  }
}

export class SignalingError extends WebRTCError {
  constructor(message: string, details?: any) {
    super(message, details);
    this.name = 'SignalingError';
  }
}

export class TransferError extends WebRTCError {
  constructor(message: string, details?: any) {
    super(message, details);
    this.name = 'TransferError';
  }
}

export class EncryptionError extends WebRTCError {
  constructor(message: string, details?: any) {
    super(message, details);
    this.name = 'EncryptionError';
  }
}
