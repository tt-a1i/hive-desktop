// Allow the isolated Hive loopback server but reject outbound Node traffic.
const net = require('node:net');
const local = host => host === undefined || ['localhost', '127.0.0.1', '::1', '[::1]'].includes(host);
const connect = net.Socket.prototype.connect;
net.Socket.prototype.connect = function (...args) {
  const first = Array.isArray(args[0]) ? args[0][0] : args[0];
  const host = first && typeof first === 'object' ? first.host :
    typeof first === 'number' && typeof args[1] === 'string' ? args[1] : undefined;
  if (!local(host)) throw Error('fixture blocks outbound network');
  return connect.apply(this, args);
};
const nativeFetch = globalThis.fetch;
globalThis.fetch = async function (input, ...rest) {
  const url = new URL(typeof input === 'string' || input instanceof URL ? input : input.url);
  if (!local(url.hostname)) throw Error('fixture blocks outbound fetch');
  return nativeFetch(input, ...rest);
};
