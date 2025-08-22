mig:
	cargo sqlx migrate run

db:
	docker compose up -d

dbd:
	docker compose down

dbp:
	docker compose down -v

dbr:
	make dbp && make db
