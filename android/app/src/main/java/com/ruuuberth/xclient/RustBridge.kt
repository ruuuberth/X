package com.ruuuberth.xclient

object RustBridge {
    init {
        System.loadLibrary("xjni")
    }

    external fun ping(msg: String): String
}
