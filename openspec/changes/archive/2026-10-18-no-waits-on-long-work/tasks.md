## 1. Token refresh

- [x] 1.1 Try the refresh lock; a run that does not get it waits for the stored token, bounded
- [x] 1.2 Test: a send during a slow refresh is sent on its first attempt, with one refresh

## 2. Batches

- [x] 2.1 `events replay` in batches of 5,000, a transaction each; test with batches of two
- [x] 2.2 Identity prune deletes in batches of 5,000; test with batches of two

## 3. Jobs

- [x] 3.1 Test: a job's transaction allows its statements 5 minutes
