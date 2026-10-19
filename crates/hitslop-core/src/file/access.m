// OS calls only. Ownership, recovery and publication policy live in Rust.
#import <Foundation/Foundation.h>
#include <copyfile.h>
#include <xattr_flags.h>

char *publication_claim(const char *path, bool writing, void *context, void (*access)(void *, const char *)) {
    @autoreleasepool {
        NSURL *url = [NSURL fileURLWithFileSystemRepresentation:path isDirectory:NO relativeToURL:nil];
        NSFileCoordinator *coordinator = [[NSFileCoordinator alloc] initWithFilePresenter:nil];
        __block BOOL called = NO;
        void (^block)(NSURL *) = ^(NSURL *resolved) {
            called = YES;
            access(context, resolved.fileSystemRepresentation);
        };
        NSError *error = nil;
        @try {
            if (writing) [coordinator coordinateWritingItemAtURL:url options:NSFileCoordinatorWritingForReplacing error:&error byAccessor:block];
            else [coordinator coordinateReadingItemAtURL:url options:NSFileCoordinatorReadingWithoutChanges error:&error byAccessor:block];
        } @catch (NSException *exception) { return strdup(exception.reason.UTF8String ?: "Coordination exception"); }
        if (error) return strdup(error.localizedDescription.UTF8String);
        return called ? NULL : strdup("Accessor did not run");
    }
}

static int metadata_filter(int what, int stage, copyfile_state_t state, const char *src, const char *dst, void *context) {
    (void)src; (void)dst; (void)context;
    if (what == COPYFILE_COPY_XATTR && stage == COPYFILE_START) {
        const char *name = NULL;
        if (copyfile_state_get(state, COPYFILE_STATE_XATTRNAME, &name) == 0 && name &&
            !xattr_preserve_for_intent(name, XATTR_OPERATION_INTENT_SAVE)) return COPYFILE_SKIP;
    }
    return COPYFILE_CONTINUE;
}
// MUST run before any public-file SQLite transaction is opened. copyfile can
// open/close descriptors, which would release that process's SQLite POSIX locks.
char *publication_metadata(const char *source, const char *destination) {
    copyfile_state_t state = copyfile_state_alloc();
    if (!state) return strdup("Cannot allocate metadata state");
    copyfile_state_set(state, COPYFILE_STATE_STATUS_CB, (void *)metadata_filter);
    int status = copyfile(source, destination, state, COPYFILE_METADATA | COPYFILE_NOFOLLOW);
    int failure = errno;
    copyfile_state_free(state);
    return status ? strdup(strerror(failure)) : NULL;
}

#include <sys/clonefile.h>
int publication_clone(const char *source, const char *destination) {
    return clonefile(source, destination, CLONE_NOFOLLOW | CLONE_NOOWNERCOPY) == 0 ? 0 : errno;
}
char *publication_replacement_directory(const char *target) {
    @autoreleasepool {
        NSURL *url = [NSURL fileURLWithFileSystemRepresentation:target isDirectory:NO relativeToURL:nil];
        NSURL *directory = [[NSFileManager defaultManager] URLForDirectory:NSItemReplacementDirectory inDomain:NSUserDomainMask appropriateForURL:url create:YES error:nil];
        return directory ? strdup(directory.fileSystemRepresentation) : NULL;
    }
}
char *publication_replace(const char *original, const char *candidate, char **result) {
    @autoreleasepool {
        NSURL *old = [NSURL fileURLWithFileSystemRepresentation:original isDirectory:NO relativeToURL:nil];
        NSURL *new = [NSURL fileURLWithFileSystemRepresentation:candidate isDirectory:NO relativeToURL:nil];
        NSURL *actual = nil;
        NSError *error = nil;
        BOOL ok = [[NSFileManager defaultManager] replaceItemAtURL:old withItemAtURL:new backupItemName:nil options:0 resultingItemURL:&actual error:&error];
        if (!ok) return strdup(error.description.UTF8String ?: "Foundation replacement failed; inspect pending publication");
        *result = strdup((actual ?: old).fileSystemRepresentation);
        return NULL;
    }
}

// Rust resolves the account home with getpwuid_r and refuses CloudStorage domains
// before this call. Ubiquity catches Desktop/Documents sync without assuming a symlink.
bool publication_ubiquitous(const char *path) {
    @autoreleasepool {
        NSURL *url = [NSURL fileURLWithFileSystemRepresentation:path isDirectory:NO relativeToURL:nil];
        while (url.path.length > 1) {
            NSNumber *value = nil;
            if ([url getResourceValue:&value forKey:NSURLIsUbiquitousItemKey error:nil] && value.boolValue) return true;
            url = url.URLByDeletingLastPathComponent;
        }
        return false;
    }
}
