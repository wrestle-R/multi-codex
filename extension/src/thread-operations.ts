/** Ordered per-thread mutations; a failed request never poisons later work. */
export class ThreadOperations {
  private tails = new Map<string, Promise<unknown>>();
  run<T>(id: string, action: () => Promise<T>): Promise<T> {
    const previous = this.tails.get(id) ?? Promise.resolve();
    const result = previous.catch(() => {}).then(action);
    this.tails.set(id, result);
    void result.finally(() => { if (this.tails.get(id) === result) this.tails.delete(id); }).catch(() => {});
    return result;
  }
  async drain() { await Promise.all([...this.tails.values()].map(pending => pending.catch(() => {}))); }
}
