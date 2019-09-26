# How to enable SSL for SpO₂

SpO₂ Doesn't support SSL by itself, it is why we used an ngninx on top of it.
Configuring an Nginx to SSL encrypt the HTTP server was easy but the hard part was to encrypt WebSockets.

To make it work you must install CertBot first, it will generate certificates and auto renew them.
Once you have installed CertBot it will automatically manage your certificates and make your domain access secure.

https://certbot.eff.org/lets-encrypt/debianbuster-nginx

Here is an example of our own Nginx configuration after CertBot have been installed:

```Nginx
server {
    listen 80;
    return 301 https://$host$request_uri;
}

server {
    listen 443;
    server_name spo2.yourdomainname.com;
    ssl_certificate /etc/letsencrypt/live/spo2.yourdomainname.com/fullchain.pem; # managed by Certbot
    ssl_certificate_key /etc/letsencrypt/live/spo2.yourdomainname.com/privkey.pem; # managed by Certbot

    ssl on;
    ssl_session_cache  builtin:1000  shared:SSL:10m;
    ssl_protocols  TLSv1 TLSv1.1 TLSv1.2;
    ssl_ciphers HIGH:!aNULL:!eNULL:!EXPORT:!CAMELLIA:!DES:!MD5:!PSK:!RC4;
    ssl_prefer_server_ciphers on;

    access_log                  /var/log/nginx/spo2.access.log;

    location / {
    auth_basic "Please enter the secret password";
    auth_basic_user_file /etc/apache2/.htpasswd;

        proxy_set_header        Host $host;
        proxy_set_header        X-Real-IP $remote_addr;
        proxy_set_header        X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header        X-Forwarded-Proto $scheme;

        proxy_pass          http://localhost:8000;
        proxy_read_timeout  90;

        proxy_redirect      http://localhost:8000 https://spo2.yourdomainname.com;
    }

}

upstream appserver {
    server localhost:8001;
}

server {
    listen 8888;
    # server_name spo2.yourdomainname.com;

    ssl on;
    ssl_certificate /etc/letsencrypt/live/spo2.yourdomainname.com/fullchain.pem; # managed by Certbot
    ssl_certificate_key /etc/letsencrypt/live/spo2.yourdomainname.com/privkey.pem; # managed by Certbot

    access_log                  /var/log/nginx/spo2.access.log;

    location / {
        proxy_pass          http://appserver;
        proxy_read_timeout  90;

        proxy_http_version      1.1;
        proxy_set_header        Upgrade $http_upgrade;
        proxy_set_header        Connection "upgrade";
    }
}
```
