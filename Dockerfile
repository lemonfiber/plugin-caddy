FROM caddy:2.11.7@sha256:f2a1290d0463aad60660d4ec134943f183ee2a5f6c3eb7bf32dd984f2f020772
RUN apk add --no-cache libcap \
 && setcap -r /usr/bin/caddy \
 && apk del --no-cache libcap
USER 1000:1000
