import admin from 'firebase-admin';

import serviceAccount from '$lib/server/firebase/motis-project-cbb85-cbf312fcb3d6.json';

// Check if already initialized to prevent "already exists" error in dev mode
if (!admin.apps.length) {
    admin.initializeApp({
        credential: admin.credential.cert(serviceAccount as admin.ServiceAccount)
    });
}

export const adminAuth = admin.auth();
export const adminMessaging = admin.messaging();
export const adminDb = admin.firestore();
