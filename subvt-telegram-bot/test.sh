#!/bin/bash
set -euo pipefail

function cleanup()
{
    echo "🧹 Clean up resources."
    echo "🔽 Stop and remove the app PostgreSQL container and volume."
    docker stop subvt_test_app_postgres
    docker rm subvt_test_app_postgres
    docker volume rm subvt_test_app_postgres_data
    echo "🔽 Stop and remove the network Redis container."
    docker stop subvt_test_network_redis
    docker rm subvt_test_network_redis
    docker volume rm subvt_test_network_redis_data
    echo "🔽 Stop and remove the network PostgreSQL container and volume."
    docker stop subvt_test_network_postgres
    docker rm subvt_test_network_postgres
    docker volume rm subvt_test_network_postgres_data
    echo "🏁 SubVT Telegram Bot testing completed."
}

if ! docker info > /dev/null 2>&1; then
  echo "🐳 This script uses Docker, and it isn't running - please start Docker and try again!"
  exit 1
fi
trap cleanup EXIT
echo "🏗 SubVT Telegram Bot setup started."
echo "🔼 Start the network Redis container and volume."
docker run --name subvt_test_network_redis --platform linux/amd64 -d -p 6379:6379 -v subvt_test_network_redis_data:/data redis:7.0
echo "🔼 Start the network PostgreSQL container and volume."
docker run --name subvt_test_network_postgres --platform linux/amd64 -d -p 15432:5432 -v subvt_test_network_postgres_data:/var/lib/postgresql/data helikon/subvt-network-postgres:latest
echo "🔼 Start the app PostgreSQL container and volume."
docker run --name subvt_test_app_postgres --platform linux/amd64 -d -p 25432:5432 -v subvt_test_app_postgres_data:/var/lib/postgresql/data helikon/subvt-app-postgres:latest
echo "😴 Sleep for 30 seconds to make sure that database migrations are complete..."
sleep 30
echo "🟢 Start testing."
SUBVT_ENV=test cargo test -- --show-output --test-threads=1
echo "✅ Testing completed successfully."
exit 0
