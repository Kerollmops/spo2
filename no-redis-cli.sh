#!/bin/bash

set -e

if [ $# -eq 0 ]; then
    echo "Usage: $0 [insert|remove] url"
    exit 1
fi

REDIS_HOST=${REDIS_HOST:-'127.0.0.1'}
REDIS_PORT=${REDIS_PORT:-'6379'}

echo "Redis connection used is $REDIS_HOST:$REDIS_PORT"

url=$2
len=${#url}

if [ "$1" = "insert" ]; then
    # insert the pod into spo2
    echo -ne "*2\r\n\$11\r\nspo2.insert\r\n\$$len\r\n$url\r\n" | nc $REDIS_HOST $REDIS_PORT
elif [ "$1" =  "remove" ]; then
    # remove the pod from spo2
    echo -ne "*2\r\n\$11\r\nspo2.remove\r\n\$$len\r\n$url\r\n" | nc $REDIS_HOST $REDIS_PORT
else
    echo "Invalid command \"$1\", expected \"insert\" or \"remove\""
    exit 1
fi
