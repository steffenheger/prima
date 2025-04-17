package de.motis.prima.services

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import android.util.Log
import androidx.core.app.NotificationCompat
import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage
import de.motis.prima.MainActivity
import de.motis.prima.R

class FirebaseService: FirebaseMessagingService() {
    override fun onMessageReceived(remoteMessage: RemoteMessage) {
        super.onMessageReceived(remoteMessage)

        val data = remoteMessage.data
        val tourId = data["tourId"]
        val title = remoteMessage.notification?.title ?: "Default Title"
        val body = remoteMessage.notification?.body ?: "Default Body"

        if (data.isNotEmpty()) {
            showNotification(title, body, tourId)
        }
    }

    override fun onNewToken(token: String) {
        super.onNewToken(token)
        // Send token to your server
        Log.d("test", "New FCM token: $token")
    }

    private fun showNotification(title: String?, body: String?, tourId: String?) {
        val intent = Intent(this, MainActivity::class.java).apply {
            putExtra("tourId", tourId)
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK
        }
        val pendingIntent = PendingIntent.getActivity(
            this,
            0,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // Ensure the Notification Channel is created for Android Oreo and above
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channelId = "default_channel_id"
            val channelName = "Default Channel"
            val channel = NotificationChannel(channelId, channelName, NotificationManager.IMPORTANCE_DEFAULT).apply {
                description = "Channel description"
            }
            val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            notificationManager.createNotificationChannel(channel)
        }

        val notification = NotificationCompat.Builder(this, "default_channel_id")
            .setContentTitle(title)
            .setContentText(body)
            .setSmallIcon(R.drawable.ic_bell)
            .setContentIntent(pendingIntent)
            .setAutoCancel(true)
            .build()

        val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        notificationManager.notify(0, notification)
    }
}