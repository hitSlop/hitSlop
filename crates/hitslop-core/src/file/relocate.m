#import <Foundation/Foundation.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>

// Synchronous OS accessors only. Rust retains the lease, decides when copying is safe,
// and checks the source and destination again before authorizing the trash operation.
bool hitslop_relocation_access(const char *path, bool deleting,
                               bool (*accessor)(const char *, void *), void *context,
                               char **message) {
    @autoreleasepool {
        NSURL *url = [NSURL fileURLWithFileSystemRepresentation:path isDirectory:NO relativeToURL:nil];
        NSFileCoordinator *coordinator = [[NSFileCoordinator alloc] initWithFilePresenter:nil];
        NSError *error = nil;
        __block bool accepted = false;
        void (^body)(NSURL *) = ^(NSURL *resolved) {
            accepted = accessor(resolved.fileSystemRepresentation, context);
        };
        if (deleting) {
            [coordinator coordinateWritingItemAtURL:url options:NSFileCoordinatorWritingForDeleting
                                              error:&error byAccessor:body];
        } else {
            [coordinator coordinateReadingItemAtURL:url options:0 error:&error byAccessor:body];
        }
        if (error) *message = strdup(error.localizedDescription.UTF8String);
        return accepted && !error;
    }
}

bool hitslop_relocation_trash(const char *path, char **message) {
    @autoreleasepool {
        NSURL *url = [NSURL fileURLWithFileSystemRepresentation:path isDirectory:NO relativeToURL:nil];
        NSError *error = nil;
        bool ok = [[NSFileManager defaultManager] trashItemAtURL:url resultingItemURL:nil error:&error];
        if (error) *message = strdup(error.localizedDescription.UTF8String);
        return ok;
    }
}
