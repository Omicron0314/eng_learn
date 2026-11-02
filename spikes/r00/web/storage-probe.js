// Isolated, original data. Run via the probe page; never uses an existing profile.
export async function storageProbe() {
  const name = `cijian-r00-${crypto.randomUUID()}`;
  const open = () => new Promise((resolve, reject) => {
    const request = indexedDB.open(name, 1);
    request.onupgradeneeded = () => {
      for (const store of ['cards', 'events', 'receipts']) request.result.createObjectStore(store);
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
  const transact = (db, action) => new Promise((resolve, reject) => {
    const transaction = db.transaction(['cards', 'events', 'receipts'], 'readwrite');
    transaction.oncomplete = resolve;
    transaction.onabort = () => reject(transaction.error || new Error('aborted'));
    action(transaction);
  });
  const read = (db, store, key) => new Promise((resolve, reject) => {
    const request = db.transaction(store).objectStore(store).get(key);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
  let db;
  try {
    db = await open();
    await transact(db, tx => {
      tx.objectStore('cards').add({revision: 1}, 'original');
      tx.objectStore('events').add({card: 'original'}, 'event');
      tx.objectStore('receipts').add('saved', 'operation');
    });
    let rolledBack = false;
    try {
      await transact(db, tx => {
        tx.objectStore('cards').put({revision: 2}, 'original');
        tx.objectStore('events').add({card: 'original'}, 'event');
      });
    } catch { rolledBack = true; }
    db.close();
    db = await open();
    const card = await read(db, 'cards', 'original');
    const event = await read(db, 'events', 'event');
    const receipt = await read(db, 'receipts', 'operation');
    if (!rolledBack || card?.revision !== 1 || event?.card !== 'original' || receipt !== 'saved') {
      throw new Error('IndexedDB atomicity/reopen contract failed');
    }
    return {atomicCommit: true, failedEventRollsBackBusiness: true, reopenPreservesData: true};
  } finally {
    db?.close();
    await new Promise((resolve, reject) => {
      const request = indexedDB.deleteDatabase(name);
      request.onsuccess = resolve;
      request.onerror = () => reject(request.error);
      request.onblocked = () => reject(new Error('probe cleanup blocked'));
    });
  }
}
