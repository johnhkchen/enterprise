package dev.b28.enterprise.tool;

import java.io.DataInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * The JVM methods' bytecode sizes in a class file, read from its Code attributes: HotSpot doesn't JIT-compile a method
 * of more than 8,000 bytes ({@code -XX:+DontCompileHugeMethods}, the default), so a wasm function that compiles to one
 * runs in the JVM's bytecode interpreter, however hot.
 */
final class MethodSizes {
    /** HotSpot's {@code HugeMethodLimit}. */
    static final int HUGE = 8000;

    record Method(String name, int bytes) {}

    private MethodSizes() {}

    static List<Method> of(Path classFile) throws IOException {
        try (InputStream raw = Files.newInputStream(classFile); DataInputStream in = new DataInputStream(raw)) {
            in.readInt(); // magic
            in.readUnsignedShort();
            in.readUnsignedShort();
            int count = in.readUnsignedShort();
            String[] utf8 = new String[count];
            for (int i = 1; i < count; i++) {
                int tag = in.readUnsignedByte();
                switch (tag) {
                    case 1 -> utf8[i] = in.readUTF();
                    case 3, 4 -> in.readInt();
                    case 5, 6 -> {
                        in.readLong();
                        i++;
                    }
                    case 7, 8, 16, 19, 20 -> in.readUnsignedShort();
                    case 9, 10, 11, 12, 17, 18 -> in.readInt();
                    case 15 -> {
                        in.readUnsignedByte();
                        in.readUnsignedShort();
                    }
                    default -> throw new IOException("constant tag " + tag + " in " + classFile);
                }
            }
            in.readUnsignedShort(); // access
            in.readUnsignedShort(); // this
            in.readUnsignedShort(); // super
            int interfaces = in.readUnsignedShort();
            for (int i = 0; i < interfaces; i++) {
                in.readUnsignedShort();
            }
            skipMembers(in);
            List<Method> methods = new ArrayList<>();
            int n = in.readUnsignedShort();
            for (int i = 0; i < n; i++) {
                in.readUnsignedShort();
                String name = utf8[in.readUnsignedShort()];
                in.readUnsignedShort();
                int attributes = in.readUnsignedShort();
                for (int a = 0; a < attributes; a++) {
                    String attr = utf8[in.readUnsignedShort()];
                    int len = in.readInt();
                    if (attr.equals("Code")) {
                        in.readUnsignedShort();
                        in.readUnsignedShort();
                        int code = in.readInt();
                        methods.add(new Method(name, code));
                        in.skipNBytes(len - 8);
                    } else {
                        in.skipNBytes(len);
                    }
                }
            }
            return methods;
        }
    }

    private static void skipMembers(DataInputStream in) throws IOException {
        int n = in.readUnsignedShort();
        for (int i = 0; i < n; i++) {
            in.readUnsignedShort();
            in.readUnsignedShort();
            in.readUnsignedShort();
            int attributes = in.readUnsignedShort();
            for (int a = 0; a < attributes; a++) {
                in.readUnsignedShort();
                in.skipNBytes(in.readInt());
            }
        }
    }
}
