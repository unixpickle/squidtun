# squidtun

This will be a small tool for tunneling TCP over a [Squid](https://en.wikipedia.org/wiki/Squid_%28software%29) HTTP proxy.

Let's say you have a Squid proxy listening at `172.19.134.2:3128`. If you connect to it and make an HTTP request for a different host, the proxy will make the request on your behalf. Thus, you just need a website listening on HTTP that allows you to proxy arbitrary connections.

# Motivation

On airplanes that use gogoinflight to charge for on-board internet, there is a caching HTTP proxy on the subnet that you can access without paying for WiFi. To get the IP and port of this server, do the following:

```
curl -I http://airborne.gogoinflight.com | grep X-Cache-Lookup: | cut -f 4 -d ' '
```

On my last flight, the IP was `172.19.134.2:3128`. Perhaps this is always the IP of the proxy.

# Usage

The server listens on HTTP and serves a TCP-over-HTTP API to forward connections to a remote host. In this example, the server will proxy connections to `127.0.0.1:22` (SSH). We bind the server to port 80, but any port can be used.

```
$ squidtun-server --remote 127.0.0.1:22 --password hello 0.0.0.0:80
```

The client listens on a local TCP port and proxies connections through the server. For example, we could make the client listen on `localhost:2222` and forward the connections to our proxy's SSH server. In this example, we have the squid proxy running on `172.19.134.2:3128` and our the server is accessible via `proxy.com`.

```
$ squidtun-client --local-address 127.0.0.1:2222 --password hello 172.19.134.2:3128 proxy.com
```

Now that the client is running, we can SSH to our local machine and have the connection forwarded to the server. For example:

```
$ ssh -p 2222 user@localhost
```
