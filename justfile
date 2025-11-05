alias m := migrate

_default:
  @just -l

# migrate
migrate:
	cargo sqlx migrate run

# start db
db-up:
	docker compose up -d
alias db := db-up

# stop db
db-down:
	docker compose down
alias dbd := db-down

# stop and remove db
db-purge:
	docker compose down -v
alias dbp := db-purge

# reset db
db-reset:
	make dbp && make db
alias dbr := db-reset
