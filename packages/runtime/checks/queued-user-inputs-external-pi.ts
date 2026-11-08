/** Explicit original-CLI variant; not required on machines without Pi installed. */
process.env.HIVE_QUEUE_BACKEND = 'pi-rpc';
await import('./queued-user-inputs.js');
export {};
