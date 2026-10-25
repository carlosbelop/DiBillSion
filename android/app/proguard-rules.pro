# JNA and the UniFFI bindings are reached through reflection and JNI.
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-keep class com.dibillsion.core.** { *; }
-dontwarn java.awt.**
