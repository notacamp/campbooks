## 1. Limits

- [x] 1.1 Runtime connections start with statement_timeout 30 s and lock_timeout 10 s
- [x] 1.2 A job's own transaction allows 5 minutes per statement
- [x] 1.3 The tests' runtime-role pool gets the same limits

## 2. Errors

- [x] 2.1 57014, 55P03 and a pool acquire timeout answer 503 `database_unavailable`, logged by kind

## 3. Tests

- [x] 3.1 The settings on runtime connections, the test pool and a job's transaction
- [x] 3.2 A statement past its time and a lock wait past its time are 503s; a pool timeout is a 503
