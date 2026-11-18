// Proposer un fichier à l'utilisateur (téléchargement par le navigateur).

export function telecharger(contenu: Blob | Uint8Array | string, nom: string, type = 'application/octet-stream'): void {
  const blob = contenu instanceof Blob ? contenu : new Blob([contenu as BlobPart], { type })
  const url = URL.createObjectURL(blob)
  const lien = document.createElement('a')
  lien.href = url
  lien.download = nom
  document.body.append(lien)
  lien.click()
  lien.remove()
  setTimeout(() => URL.revokeObjectURL(url), 1000)
}
