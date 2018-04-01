# squidtun

This will be a small tool for tunneling TCP over a [Squid](https://en.wikipedia.org/wiki/Squid_%28software%29) HTTP proxy.

Let's say you have a Squid proxy listening at `172.19.134.2:3128`. If you connect to it and make an HTTP request for a different host, the proxy will make the request on your behalf. Thus, you just need a website listening on HTTP that allows you to proxy arbitrary connections.

# Motivation

On airplanes that use gogoinflight to charge for on-board internet, there is a caching HTTP proxy on the subnet that you can access without paying for WiFi. To get the IP and port of this server, do the following:

```
curl -I http://airborne.gogoinflight.com | grep X-Cache-Lookup: | cut -f 4 -d ' '
```

On my last flight, the IP was `172.19.134.2:3128`. Perhaps this is always the IP of the proxy.
