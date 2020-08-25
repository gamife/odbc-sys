use crate::{handles::Statement, ColumnDescription, Error};
use odbc_sys::{CDataType, Len, Pointer, SmallInt, UInteger, ULen, USmallInt, SqlDataType};
use std::thread::panicking;

pub struct Cursor<'open_connection> {
    statement: Statement<'open_connection>,
}

impl<'o> Drop for Cursor<'o> {
    fn drop(&mut self) {
        if let Err(e) = self.statement.close_cursor() {
            // Avoid panicking, if we already have a panic. We don't want to mask the original
            // error.
            if !panicking() {
                panic!("Unexepected error disconnecting: {:?}", e)
            }
        }
    }
}

impl<'o> Cursor<'o> {
    pub(crate) fn new(statement: Statement<'o>) -> Self {
        Self { statement }
    }

    /// Fetch a column description using the column index.
    ///
    /// # Parameters
    ///
    /// * `column_number`: Column index. `0` is the bookmark column. The other column indices start
    /// with `1`.
    /// * `column_description`: Holds the description of the column after the call. This method does
    /// not provide strong exception safety as the value of this argument is undefined in case of an
    /// error.
    pub fn describe_col(
        &self,
        column_number: USmallInt,
        column_description: &mut ColumnDescription,
    ) -> Result<(), Error> {
        self.statement
            .describe_col(column_number, column_description)?;
        Ok(())
    }

    /// Number of columns in result set.
    pub fn num_result_cols(&self) -> Result<SmallInt, Error> {
        self.statement.num_result_cols()
    }

    /// Returns the next rowset in the result set.
    ///
    /// If any columns are bound, it returns the data in those columns. If the application has
    /// specified a pointer to a row status array or a buffer in which to return the number of rows
    /// fetched, `fetch` also returns this information. Calls to `fetch` can be mixed with calls to
    /// `fetch_scroll`.
    pub fn fetch(&mut self) -> Result<bool, Error> {
        self.statement.fetch()
    }

    /// Sets the batch size for bulk cursors, if retrieving many rows at once.
    ///
    /// # Safety
    ///
    /// It is the callers responsibility to ensure that buffers bound using `bind_col` can hold the
    /// specified amount of rows.
    pub unsafe fn set_row_array_size(&mut self, size: UInteger) -> Result<(), Error> {
        self.statement.set_row_array_size(size)
    }

    /// Bind an integer to hold the number of rows retrieved with fetch in the current row set.
    ///
    /// # Safety
    ///
    /// `num_rows` must not be moved and remain valid, as long as it remains bound to the cursor.
    pub unsafe fn set_num_rows_fetched(&mut self, num_rows: &mut ULen) -> Result<(), Error> {
        self.statement.set_num_rows_fetched(num_rows)
    }

    /// Sets the binding type to columnar binding for batch cursors.
    ///
    /// Any Positive number indicates a row wise binding with that row length. `0` indicates a
    /// columnar binding.
    ///
    /// # Safety
    ///
    /// It is the callers responsibility to ensure that the bound buffers match the memory layout
    /// specified by this function.
    pub unsafe fn set_row_bind_type(&mut self, row_size: u32) -> Result<(), Error> {
        self.statement.set_row_bind_type(row_size)
    }

    /// Release all columen buffers bound by `bind_col`. Except bookmark column.
    pub fn unbind_cols(&mut self) -> Result<(), Error> {
        self.statement.unbind_cols()
    }

    /// Binds application data buffers to columns in the result set
    ///
    /// * `column_number`: `0` is the bookmark column. It is not included in some result sets. All
    /// other columns are numbered starting with `1`. It is an error to bind a higher-numbered
    /// column than there are columns in the result set. This error cannot be detected until the
    /// result set has been created, so it is returned by `fetch`, not `bind_col`.
    /// * `target_type`: The identifier of the C data type of the `value` buffer. When it is
    /// retrieving data from the data source with `fetch`, the driver converts the data to this
    /// type. When it sends data to the source, the driver converts the data from this type.
    /// * `target_value`: Pointer to the data buffer to bind to the column.
    /// * `target_length`: Length of target value in bytes. (Or for a single element in case of bulk
    /// aka. block fetching data).
    /// * `indicator`: Buffer is going to hold length or indicator values.
    ///
    /// # Safety
    ///
    /// It is the callers responsibility to make sure the bound columns live until they are no
    /// longer bound.
    pub unsafe fn bind_col(
        &mut self,
        column_number: USmallInt,
        target_type: CDataType,
        target_value: Pointer,
        target_length: Len,
        indicator: *mut Len,
    ) -> Result<(), Error> {
        self.statement.bind_col(
            column_number,
            target_type,
            target_value,
            target_length,
            indicator,
        )
    }

    /// `true` if a given column in a result set is unsigned or not a numeric type, `false`
    /// otherwise.
    ///
    /// `column_number`: Index of the column, starting at 1.
    pub fn is_unsigned_column(&self, column_number: USmallInt) -> Result<bool, Error> {
        self.statement.is_unsigned_column(column_number)
    }

    /// Binds this cursor to a buffer holding a row set.
    pub fn bind_row_set_buffer<'r, B>(
        &'r mut self,
        row_set_buffer: &'r mut B,
    ) -> Result<RowSetCursor<'r, 'o, B>, Error>
    where
        B: RowSetBuffer,
        'o: 'r,
    {
        unsafe {
            row_set_buffer.bind_to_cursor(self)?;
        }
        Ok(RowSetCursor::new(row_set_buffer, self))
    }

    /// SqlDataType
    ///
    /// `column_number`: Index of the column, starting at 1.
    pub fn col_data_type(&self, column_number: USmallInt) -> Result<SqlDataType, Error> {
        self.statement.col_data_type(column_number)
    }

    /// Returns the size in bytes of the columns. For variable sized types the maximum size is
    /// returned, excluding a terminating zero.
    ///
    /// `column_number`: Index of the column, starting at 1.
    pub fn col_octet_length(&self, column_number: USmallInt) -> Result<Len, Error> {
        self.statement.col_octet_length(column_number)
    }

    /// Maximum number of characters required to display data from the column.
    ///
    /// `column_number`: Index of the column, starting at 1.
    pub fn col_display_size(&self, column_number: USmallInt) -> Result<Len, Error> {
        self.statement.col_display_size(column_number)
    }
}

pub unsafe trait RowSetBuffer {
    unsafe fn bind_to_cursor(&mut self, cursor: &mut Cursor) -> Result<(), Error>;
}

pub struct RowSetCursor<'r, 'o, B> {
    buffer: &'r mut B,
    cursor: &'r mut Cursor<'o>,
}

impl<'r, 'o, B> RowSetCursor<'r, 'o, B> {
    fn new(buffer: &'r mut B, cursor: &'r mut Cursor<'o>) -> Self {
        Self { buffer, cursor }
    }

    pub fn fetch(&mut self) -> Result<Option<&B>, Error> {
        if self.cursor.fetch()? {
            Ok(Some(self.buffer))
        } else {
            Ok(None)
        }
    }
}

impl<'r, 'o, B> Drop for RowSetCursor<'r, 'o, B> {
    fn drop(&mut self) {
        if let Err(e) = self.cursor.unbind_cols() {
            // Avoid panicking, if we already have a panic. We don't want to mask the original
            // error.
            if !panicking() {
                panic!("Unexepected error unbinding columns: {:?}", e)
            }
        }
    }
}