// Où afficher les erreurs du Rust (logique pure, testée dans errors.test.ts).
//
// Les toasts recouvrent le bas de la colonne des mesures : ils ne servent
// qu'aux erreurs passagères. Pendant une reconnexion, l'erreur de l'instantané
// (`lastError`) reste posée tant que dure la coupure : elle va donc dans le
// bandeau orange, qui ne cache rien.

/** Début de toute erreur de perte de liaison (`MSG_LINK_LOST` dans session.rs). */
const LINK_LOST = "Liaison Bluetooth perdue";
/** Échec d'un essai : « Liaison Bluetooth perdue, reconnexion en cours… (cause) ». */
const RECONNECT_FAILED = /^Liaison Bluetooth perdue, reconnexion en cours… \((.+)\)$/s;

/**
 * Cause à montrer dans le bandeau de reconnexion, ou `null` quand l'erreur ne
 * dit rien de plus que le bandeau lui-même.
 */
export function reconnectDetail(lastError: string | null): string | null {
  if (!lastError || lastError === LINK_LOST) return null;
  return RECONNECT_FAILED.exec(lastError)?.[1] ?? lastError;
}
